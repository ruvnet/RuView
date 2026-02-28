"use strict";
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
Object.defineProperty(exports, "__esModule", { value: true });
exports.CapacityManager = void 0;
const child_process_1 = require("child_process");
const util_1 = require("util");
const burst_predictor_1 = require("./burst-predictor");
const reactive_scaler_1 = require("./reactive-scaler");
const execAsync = (0, util_1.promisify)(child_process_1.exec);
class CapacityManager {
    constructor(regions = ['us-central1', 'europe-west1', 'asia-east1'], notifyHook = async (msg) => {
        await execAsync(`npx claude-flow@alpha hooks notify --message "${msg.replace(/"/g, '\\"')}"`);
    }) {
        this.notifyHook = notifyHook;
        this.regionCapacities = new Map();
        this.trafficPriorities = new Map();
        this.isPreWarming = false;
        this.currentDegradationLevel = 'none';
        // Initialize region capacities
        this.initializeRegionCapacities(regions);
        // Initialize budget config
        this.budgetConfig = {
            hourlyBudget: 10000, // $10k/hour
            dailyBudget: 200000, // $200k/day
            monthlyBudget: 5000000, // $5M/month
            currentHourlyCost: 0,
            currentDailyCost: 0,
            currentMonthlyCost: 0,
            warningThreshold: 0.8, // Warn at 80%
            hardLimit: false // Allow temporary overages
        };
        // Initialize traffic priorities
        this.trafficPriorities.set('premium', {
            tier: 'premium',
            connectionLimit: -1, // Unlimited
            canShed: false,
            latencySLA: 30 // 30ms
        });
        this.trafficPriorities.set('standard', {
            tier: 'standard',
            connectionLimit: 1000000000,
            canShed: false,
            latencySLA: 50 // 50ms
        });
        this.trafficPriorities.set('free', {
            tier: 'free',
            connectionLimit: 100000000,
            canShed: true,
            latencySLA: 200 // 200ms
        });
        // Initialize predictor and scaler
        this.predictor = new burst_predictor_1.BurstPredictor(regions, notifyHook);
        this.scaler = new reactive_scaler_1.ReactiveScaler(regions, notifyHook);
    }
    /**
     * Initialize region capacities with costs
     */
    initializeRegionCapacities(regions) {
        const costMap = {
            'us-central1': 0.50, // $0.50/hour per instance
            'us-east1': 0.52,
            'us-west1': 0.54,
            'europe-west1': 0.55,
            'europe-west4': 0.58,
            'asia-east1': 0.60,
            'asia-southeast1': 0.62,
            'south-america-east1': 0.65
        };
        const priorityMap = {
            'us-central1': 10, // Highest priority
            'us-east1': 9,
            'europe-west1': 9,
            'asia-east1': 8,
            'us-west1': 7,
            'asia-southeast1': 6,
            'europe-west4': 6,
            'south-america-east1': 5
        };
        for (const region of regions) {
            this.regionCapacities.set(region, {
                region,
                currentInstances: 10, // Start with min instances
                maxInstances: 1000,
                availableInstances: 990,
                costPerInstance: costMap[region] || 0.50,
                priority: priorityMap[region] || 5
            });
        }
    }
    /**
     * Update budget configuration
     */
    updateBudget(config) {
        this.budgetConfig = { ...this.budgetConfig, ...config };
    }
    /**
     * Main orchestration loop
     */
    async orchestrate() {
        // 1. Get predictions
        const predictions = await this.predictor.predictUpcomingBursts(24);
        // 2. Check if pre-warming is needed
        if (predictions.length > 0 && !this.isPreWarming) {
            await this.handlePreWarming(predictions);
        }
        // 3. Process reactive scaling for each region
        const scalingActions = [];
        for (const [region, capacity] of this.regionCapacities) {
            // Get current metrics (in production, fetch from monitoring)
            const metrics = await this.getCurrentMetrics(region);
            // Process reactive scaling
            const action = await this.scaler.processMetrics(metrics);
            if (action.action !== 'none') {
                scalingActions.push(action);
            }
        }
        // 4. Apply scaling actions with budget constraints
        await this.applyScalingActions(scalingActions);
        // 5. Check budget and apply degradation if needed
        await this.checkBudgetAndDegrade();
        // 6. Generate capacity plan
        return this.generateCapacityPlan();
    }
    /**
     * Handle pre-warming for predicted bursts
     */
    async handlePreWarming(predictions) {
        const now = new Date();
        for (const prediction of predictions) {
            const preWarmTime = new Date(prediction.startTime.getTime() - prediction.preWarmTime * 1000);
            if (now >= preWarmTime && now < prediction.startTime) {
                this.isPreWarming = true;
                await this.notifyHook(`PRE-WARMING: Starting capacity ramp-up for ${prediction.eventName} (${prediction.expectedMultiplier}x load expected)`);
                // Scale each region to required capacity
                for (const regionPred of prediction.regions) {
                    const capacity = this.regionCapacities.get(regionPred.region);
                    if (capacity && regionPred.requiredInstances > capacity.currentInstances) {
                        await this.scaleRegion(regionPred.region, regionPred.requiredInstances, 'predictive-prewarm');
                    }
                }
            }
        }
    }
    /**
     * Apply scaling actions with budget and priority constraints
     */
    async applyScalingActions(actions) {
        // Sort by urgency and priority
        const sortedActions = actions.sort((a, b) => {
            const urgencyScore = { critical: 4, high: 3, normal: 2, low: 1 };
            const aScore = urgencyScore[a.urgency];
            const bScore = urgencyScore[b.urgency];
            if (aScore !== bScore)
                return bScore - aScore;
            // Then by region priority
            const aCapacity = this.regionCapacities.get(a.region);
            const bCapacity = this.regionCapacities.get(b.region);
            return bCapacity.priority - aCapacity.priority;
        });
        for (const action of sortedActions) {
            if (action.action === 'scale-out') {
                // Check budget before scaling out
                const canScale = await this.checkBudgetForScaling(action.region, action.toInstances - action.fromInstances);
                if (canScale) {
                    await this.scaleRegion(action.region, action.toInstances, 'reactive');
                }
                else {
                    await this.notifyHook(`BUDGET LIMIT: Cannot scale ${action.region} - budget exceeded`);
                    // Consider degradation
                    await this.applyDegradation('minor');
                }
            }
            else if (action.action === 'scale-in') {
                // Always allow scale-in (saves money)
                await this.scaleRegion(action.region, action.toInstances, 'reactive');
            }
        }
    }
    /**
     * Scale a specific region
     */
    async scaleRegion(region, targetInstances, reason) {
        const capacity = this.regionCapacities.get(region);
        if (!capacity) {
            throw new Error(`Region ${region} not found`);
        }
        const oldInstances = capacity.currentInstances;
        capacity.currentInstances = Math.min(targetInstances, capacity.maxInstances);
        capacity.availableInstances = capacity.maxInstances - capacity.currentInstances;
        // Update budget
        await this.updateBudgetCosts();
        await this.notifyHook(`SCALED: ${region} ${oldInstances} -> ${capacity.currentInstances} instances (${reason})`);
        // In production, call Terraform or Cloud Run API to actually scale
        // await this.executeTerraformScale(region, capacity.currentInstances);
    }
    /**
     * Check if budget allows scaling
     */
    async checkBudgetForScaling(region, additionalInstances) {
        const capacity = this.regionCapacities.get(region);
        const additionalCost = capacity.costPerInstance * additionalInstances;
        const newHourlyCost = this.budgetConfig.currentHourlyCost + additionalCost;
        if (this.budgetConfig.hardLimit) {
            // Hard limit - don't exceed budget
            return newHourlyCost <= this.budgetConfig.hourlyBudget;
        }
        else {
            // Soft limit - warn but allow
            if (newHourlyCost > this.budgetConfig.hourlyBudget * this.budgetConfig.warningThreshold) {
                await this.notifyHook(`BUDGET WARNING: Approaching hourly budget limit ($${newHourlyCost.toFixed(2)}/$${this.budgetConfig.hourlyBudget})`);
            }
            // Allow up to 120% of budget for burst events
            return newHourlyCost <= this.budgetConfig.hourlyBudget * 1.2;
        }
    }
    /**
     * Update budget costs based on current capacity
     */
    async updateBudgetCosts() {
        let totalHourlyCost = 0;
        for (const capacity of this.regionCapacities.values()) {
            totalHourlyCost += capacity.currentInstances * capacity.costPerInstance;
        }
        this.budgetConfig.currentHourlyCost = totalHourlyCost;
        this.budgetConfig.currentDailyCost = totalHourlyCost * 24;
        this.budgetConfig.currentMonthlyCost = totalHourlyCost * 24 * 30;
    }
    /**
     * Check budget and apply degradation if needed
     */
    async checkBudgetAndDegrade() {
        const hourlyUsage = this.budgetConfig.currentHourlyCost / this.budgetConfig.hourlyBudget;
        const dailyUsage = this.budgetConfig.currentDailyCost / this.budgetConfig.dailyBudget;
        if (hourlyUsage > 1.0 || dailyUsage > 1.0) {
            await this.applyDegradation('major');
        }
        else if (hourlyUsage > 0.9 || dailyUsage > 0.9) {
            await this.applyDegradation('minor');
        }
        else if (this.currentDegradationLevel !== 'none') {
            // Recover from degradation
            await this.applyDegradation('none');
        }
    }
    /**
     * Apply degradation strategy
     */
    async applyDegradation(level) {
        if (level === this.currentDegradationLevel) {
            return; // Already at this level
        }
        const strategy = this.getDegradationStrategy(level);
        this.currentDegradationLevel = level;
        await this.notifyHook(`DEGRADATION: ${level.toUpperCase()} - ${strategy.impactDescription}`);
        // Execute degradation actions
        for (const action of strategy.actions) {
            // In production, execute actual degradation (e.g., enable rate limiting, shed traffic)
            console.log(`Executing: ${action}`);
        }
    }
    /**
     * Get degradation strategy for a given level
     */
    getDegradationStrategy(level) {
        const strategies = {
            none: {
                level: 'none',
                actions: ['Restore normal operations'],
                impactDescription: 'Normal operations - all features available'
            },
            minor: {
                level: 'minor',
                actions: [
                    'Reduce connection limits for free tier by 20%',
                    'Increase cache TTL by 2x',
                    'Defer non-critical background jobs'
                ],
                impactDescription: 'Minor impact - free tier users may experience connection limits'
            },
            major: {
                level: 'major',
                actions: [
                    'Shed 50% of free tier traffic',
                    'Reduce connection limits for standard tier by 10%',
                    'Disable non-essential features (recommendations, analytics)',
                    'Enable aggressive connection pooling'
                ],
                impactDescription: 'Major impact - free tier heavily restricted, some features disabled'
            },
            critical: {
                level: 'critical',
                actions: [
                    'Shed all free tier traffic',
                    'Reduce standard tier to 50% capacity',
                    'Premium tier only with reduced features',
                    'Enable maintenance mode for non-critical services'
                ],
                impactDescription: 'Critical - only premium tier with limited functionality'
            }
        };
        return strategies[level];
    }
    /**
     * Generate capacity plan
     */
    generateCapacityPlan() {
        let totalInstances = 0;
        let totalCost = 0;
        const regions = [];
        for (const capacity of this.regionCapacities.values()) {
            const instances = capacity.currentInstances;
            const cost = instances * capacity.costPerInstance;
            const utilization = capacity.currentInstances / capacity.maxInstances;
            totalInstances += instances;
            totalCost += cost;
            regions.push({
                region: capacity.region,
                instances,
                cost,
                utilization
            });
        }
        const budgetRemaining = this.budgetConfig.hourlyBudget - this.budgetConfig.currentHourlyCost;
        return {
            timestamp: new Date(),
            totalInstances,
            totalCost,
            regions,
            budgetRemaining,
            degradationLevel: this.currentDegradationLevel
        };
    }
    /**
     * Get current metrics for a region (mock - would fetch from monitoring in production)
     */
    async getCurrentMetrics(region) {
        const capacity = this.regionCapacities.get(region);
        // Mock metrics - in production, fetch from Cloud Monitoring
        return {
            region,
            timestamp: new Date(),
            cpuUtilization: 0.5 + Math.random() * 0.3, // 50-80%
            memoryUtilization: 0.4 + Math.random() * 0.3, // 40-70%
            activeConnections: capacity.currentInstances * 400000 + Math.random() * 100000,
            requestRate: capacity.currentInstances * 1000,
            errorRate: 0.001 + Math.random() * 0.004, // 0.1-0.5%
            p99Latency: 30 + Math.random() * 20, // 30-50ms
            currentInstances: capacity.currentInstances
        };
    }
    /**
     * Get global capacity status
     */
    getGlobalStatus() {
        let totalInstances = 0;
        let totalCost = 0;
        for (const capacity of this.regionCapacities.values()) {
            totalInstances += capacity.currentInstances;
            totalCost += capacity.currentInstances * capacity.costPerInstance;
        }
        return {
            totalInstances,
            totalCost,
            budgetUsage: totalCost / this.budgetConfig.hourlyBudget,
            degradationLevel: this.currentDegradationLevel,
            regions: this.regionCapacities
        };
    }
}
exports.CapacityManager = CapacityManager;
// Example usage
if (require.main === module) {
    const manager = new CapacityManager();
    // Run orchestration
    manager.orchestrate().then(plan => {
        console.log('\n=== Capacity Plan ===');
        console.log(`Timestamp: ${plan.timestamp.toISOString()}`);
        console.log(`Total Instances: ${plan.totalInstances}`);
        console.log(`Total Cost: $${plan.totalCost.toFixed(2)}/hour`);
        console.log(`Budget Remaining: $${plan.budgetRemaining.toFixed(2)}/hour`);
        console.log(`Degradation Level: ${plan.degradationLevel}`);
        console.log('\nRegions:');
        plan.regions.forEach(r => {
            console.log(`  ${r.region}: ${r.instances} instances ($${r.cost.toFixed(2)}/hr, ${(r.utilization * 100).toFixed(1)}% utilization)`);
        });
    });
}
//# sourceMappingURL=capacity-manager.js.map