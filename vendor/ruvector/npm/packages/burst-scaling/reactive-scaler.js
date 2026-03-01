"use strict";
/**
 * Reactive Scaler - Real-time Auto-scaling
 *
 * Handles reactive scaling based on:
 * - Real-time metrics (CPU, memory, connections)
 * - Dynamic threshold adjustment
 * - Rapid scale-out (seconds)
 * - Gradual scale-in to avoid thrashing
 */
Object.defineProperty(exports, "__esModule", { value: true });
exports.ReactiveScaler = void 0;
const child_process_1 = require("child_process");
const util_1 = require("util");
const execAsync = (0, util_1.promisify)(child_process_1.exec);
class ReactiveScaler {
    constructor(regions = ['us-central1', 'europe-west1', 'asia-east1'], notifyHook = async (msg) => {
        await execAsync(`npx claude-flow@alpha hooks notify --message "${msg.replace(/"/g, '\\"')}"`);
    }) {
        this.regions = regions;
        this.notifyHook = notifyHook;
        this.lastScaleTime = new Map();
        this.metricsHistory = new Map();
        this.historySize = 60; // Keep 60 samples (5 minutes at 5s intervals)
        // Default thresholds
        this.thresholds = {
            cpuScaleOut: 0.70, // Scale out at 70% CPU
            cpuScaleIn: 0.30, // Scale in at 30% CPU
            memoryScaleOut: 0.75,
            memoryScaleIn: 0.35,
            connectionsPerInstance: 500000,
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
    updateThresholds(thresholds) {
        this.thresholds = { ...this.thresholds, ...thresholds };
    }
    /**
     * Update scaling configuration
     */
    updateConfig(config) {
        this.config = { ...this.config, ...config };
    }
    /**
     * Process metrics and determine scaling action
     */
    async processMetrics(metrics) {
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
            await this.notifyHook(`SCALING: ${action.region} ${action.action} ${action.fromInstances} -> ${action.toInstances} (${action.reason})`);
        }
        return action;
    }
    /**
     * Determine what scaling action to take based on metrics
     */
    async determineScalingAction(metrics) {
        const reasons = [];
        let shouldScaleOut = false;
        let shouldScaleIn = false;
        let urgency = 'normal';
        // Check CPU utilization
        if (metrics.cpuUtilization > this.thresholds.cpuScaleOut) {
            reasons.push(`CPU ${(metrics.cpuUtilization * 100).toFixed(1)}%`);
            shouldScaleOut = true;
            if (metrics.cpuUtilization > this.config.rapidScaleOutThreshold) {
                urgency = 'critical';
            }
            else if (metrics.cpuUtilization > 0.8) {
                urgency = 'high';
            }
        }
        else if (metrics.cpuUtilization < this.thresholds.cpuScaleIn) {
            if (this.isStableForScaleIn(metrics.region, 'cpu')) {
                shouldScaleIn = true;
            }
        }
        // Check memory utilization
        if (metrics.memoryUtilization > this.thresholds.memoryScaleOut) {
            reasons.push(`Memory ${(metrics.memoryUtilization * 100).toFixed(1)}%`);
            shouldScaleOut = true;
            urgency = urgency === 'critical' ? 'critical' : 'high';
        }
        else if (metrics.memoryUtilization < this.thresholds.memoryScaleIn) {
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
            }
            else {
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
        }
        else if (shouldScaleIn && !shouldScaleOut) {
            return this.createScaleInAction(metrics, 'Low utilization');
        }
        else {
            return this.createNoAction(metrics, 'Within thresholds');
        }
    }
    /**
     * Create scale-out action
     */
    createScaleOutAction(metrics, reason, urgency) {
        const fromInstances = metrics.currentInstances;
        // Calculate how many instances to add
        let step = this.config.scaleOutStep;
        // Rapid scaling for critical situations
        if (urgency === 'critical') {
            step = Math.ceil(fromInstances * 0.5); // Add 50% capacity
        }
        else if (urgency === 'high') {
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
    createScaleInAction(metrics, reason) {
        const fromInstances = metrics.currentInstances;
        const toInstances = Math.max(fromInstances - this.config.scaleInStep, this.config.minInstances);
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
    createNoAction(metrics, reason) {
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
    isStableForScaleIn(region, metric) {
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
    addMetricsToHistory(metrics) {
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
    getMetricsSummary() {
        const summary = new Map();
        for (const [region, history] of this.metricsHistory) {
            if (history.length === 0)
                continue;
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
    calculateRecommendedInstances(metrics) {
        // Calculate based on connections
        const connectionBased = Math.ceil(metrics.activeConnections / this.thresholds.connectionsPerInstance);
        // Calculate based on CPU (target 60% utilization)
        const cpuBased = Math.ceil((metrics.currentInstances * metrics.cpuUtilization) / 0.6);
        // Calculate based on memory (target 65% utilization)
        const memoryBased = Math.ceil((metrics.currentInstances * metrics.memoryUtilization) / 0.65);
        // Take the maximum to ensure we have enough capacity
        const recommended = Math.max(connectionBased, cpuBased, memoryBased);
        // Apply min/max constraints
        return Math.max(this.config.minInstances, Math.min(recommended, this.config.maxInstances));
    }
    /**
     * Get scaling recommendation for predictive scaling integration
     */
    async getScalingRecommendation(region) {
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
        const reasoning = [];
        if (recommended > latest.currentInstances) {
            reasoning.push(`Current load requires ${recommended} instances`);
            reasoning.push(`CPU: ${(latest.cpuUtilization * 100).toFixed(1)}%`);
            reasoning.push(`Memory: ${(latest.memoryUtilization * 100).toFixed(1)}%`);
            reasoning.push(`Connections: ${latest.activeConnections.toLocaleString()}`);
        }
        else if (recommended < latest.currentInstances) {
            reasoning.push(`Can scale down to ${recommended} instances`);
            reasoning.push('Low utilization detected');
        }
        else {
            reasoning.push('Current capacity is optimal');
        }
        return {
            currentInstances: latest.currentInstances,
            recommendedInstances: recommended,
            reasoning
        };
    }
}
exports.ReactiveScaler = ReactiveScaler;
// Example usage
if (require.main === module) {
    const scaler = new ReactiveScaler();
    // Simulate metrics
    const metrics = {
        region: 'us-central1',
        timestamp: new Date(),
        cpuUtilization: 0.85, // High CPU
        memoryUtilization: 0.72,
        activeConnections: 45000000,
        requestRate: 150000,
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
//# sourceMappingURL=reactive-scaler.js.map