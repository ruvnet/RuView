/**
 * Reactive Scaler - Real-time Auto-scaling
 *
 * Handles reactive scaling based on:
 * - Real-time metrics (CPU, memory, connections)
 * - Dynamic threshold adjustment
 * - Rapid scale-out (seconds)
 * - Gradual scale-in to avoid thrashing
 */
export interface ScalingMetrics {
    region: string;
    timestamp: Date;
    cpuUtilization: number;
    memoryUtilization: number;
    activeConnections: number;
    requestRate: number;
    errorRate: number;
    p99Latency: number;
    currentInstances: number;
}
export interface ScalingThresholds {
    cpuScaleOut: number;
    cpuScaleIn: number;
    memoryScaleOut: number;
    memoryScaleIn: number;
    connectionsPerInstance: number;
    maxP99Latency: number;
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
    scaleOutCooldown: number;
    scaleInCooldown: number;
    scaleOutStep: number;
    scaleInStep: number;
    rapidScaleOutThreshold: number;
}
export declare class ReactiveScaler {
    private readonly regions;
    private readonly notifyHook;
    private thresholds;
    private config;
    private lastScaleTime;
    private metricsHistory;
    private readonly historySize;
    constructor(regions?: string[], notifyHook?: (message: string) => Promise<void>);
    /**
     * Update scaling thresholds
     */
    updateThresholds(thresholds: Partial<ScalingThresholds>): void;
    /**
     * Update scaling configuration
     */
    updateConfig(config: Partial<ScalingConfig>): void;
    /**
     * Process metrics and determine scaling action
     */
    processMetrics(metrics: ScalingMetrics): Promise<ScalingAction>;
    /**
     * Determine what scaling action to take based on metrics
     */
    private determineScalingAction;
    /**
     * Create scale-out action
     */
    private createScaleOutAction;
    /**
     * Create scale-in action
     */
    private createScaleInAction;
    /**
     * Create no-action result
     */
    private createNoAction;
    /**
     * Check if metrics have been stable enough for scale-in
     */
    private isStableForScaleIn;
    /**
     * Add metrics to history
     */
    private addMetricsToHistory;
    /**
     * Get current metrics summary for all regions
     */
    getMetricsSummary(): Map<string, {
        avgCpu: number;
        avgMemory: number;
        avgLatency: number;
        totalConnections: number;
        instances: number;
    }>;
    /**
     * Calculate recommended instances based on current load
     */
    calculateRecommendedInstances(metrics: ScalingMetrics): number;
    /**
     * Get scaling recommendation for predictive scaling integration
     */
    getScalingRecommendation(region: string): Promise<{
        currentInstances: number;
        recommendedInstances: number;
        reasoning: string[];
    }>;
}
//# sourceMappingURL=reactive-scaler.d.ts.map