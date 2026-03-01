/**
 * Capacity Manager - Global Capacity Orchestration
 *
 * Handles:
 * - Cross-region capacity allocation
 * - Budget-aware scaling decisions
 * - Priority-based resource allocation
 * - Graceful degradation strategies
 * - Traffic shedding when necessary
 */
export interface RegionCapacity {
    region: string;
    currentInstances: number;
    maxInstances: number;
    availableInstances: number;
    costPerInstance: number;
    priority: number;
}
export interface BudgetConfig {
    hourlyBudget: number;
    dailyBudget: number;
    monthlyBudget: number;
    currentHourlyCost: number;
    currentDailyCost: number;
    currentMonthlyCost: number;
    warningThreshold: number;
    hardLimit: boolean;
}
export interface TrafficPriority {
    tier: 'premium' | 'standard' | 'free';
    connectionLimit: number;
    canShed: boolean;
    latencySLA: number;
}
export interface CapacityPlan {
    timestamp: Date;
    totalInstances: number;
    totalCost: number;
    regions: Array<{
        region: string;
        instances: number;
        cost: number;
        utilization: number;
    }>;
    budgetRemaining: number;
    degradationLevel: 'none' | 'minor' | 'major' | 'critical';
}
export interface DegradationStrategy {
    level: 'none' | 'minor' | 'major' | 'critical';
    actions: string[];
    impactDescription: string;
}
export declare class CapacityManager {
    private readonly notifyHook;
    private regionCapacities;
    private budgetConfig;
    private trafficPriorities;
    private predictor;
    private scaler;
    private isPreWarming;
    private currentDegradationLevel;
    constructor(regions?: string[], notifyHook?: (message: string) => Promise<void>);
    /**
     * Initialize region capacities with costs
     */
    private initializeRegionCapacities;
    /**
     * Update budget configuration
     */
    updateBudget(config: Partial<BudgetConfig>): void;
    /**
     * Main orchestration loop
     */
    orchestrate(): Promise<CapacityPlan>;
    /**
     * Handle pre-warming for predicted bursts
     */
    private handlePreWarming;
    /**
     * Apply scaling actions with budget and priority constraints
     */
    private applyScalingActions;
    /**
     * Scale a specific region
     */
    private scaleRegion;
    /**
     * Check if budget allows scaling
     */
    private checkBudgetForScaling;
    /**
     * Update budget costs based on current capacity
     */
    private updateBudgetCosts;
    /**
     * Check budget and apply degradation if needed
     */
    private checkBudgetAndDegrade;
    /**
     * Apply degradation strategy
     */
    private applyDegradation;
    /**
     * Get degradation strategy for a given level
     */
    private getDegradationStrategy;
    /**
     * Generate capacity plan
     */
    private generateCapacityPlan;
    /**
     * Get current metrics for a region (mock - would fetch from monitoring in production)
     */
    private getCurrentMetrics;
    /**
     * Get global capacity status
     */
    getGlobalStatus(): {
        totalInstances: number;
        totalCost: number;
        budgetUsage: number;
        degradationLevel: string;
        regions: Map<string, RegionCapacity>;
    };
}
//# sourceMappingURL=capacity-manager.d.ts.map