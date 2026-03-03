/**
 * Feedback Loop System for Behavior Modification
 * Enables the system to learn from outcomes and modify behavior dynamically
 */
export interface FeedbackSignal {
    id: string;
    source: string;
    type: 'success' | 'failure' | 'partial' | 'unexpected' | 'novel';
    action: string;
    outcome: any;
    expected: any;
    surprise: number;
    utility: number;
    timestamp: number;
    context: any;
}
export interface BehaviorModification {
    component: string;
    parameter: string;
    oldValue: any;
    newValue: any;
    reason: string;
    confidence: number;
    timestamp: number;
    expectedImprovement: number;
}
export interface AdaptationRule {
    trigger: (feedback: FeedbackSignal) => boolean;
    modification: (feedback: FeedbackSignal, currentState: any) => BehaviorModification[];
    priority: number;
    learningRate: number;
    category: string;
}
export declare class FeedbackLoopSystem {
    private feedbackHistory;
    private behaviorModifications;
    private adaptationRules;
    private behaviorParameters;
    private performanceMetrics;
    private learningCurves;
    constructor();
    /**
     * Process feedback and trigger behavior modifications
     */
    processFeedback(feedback: FeedbackSignal): Promise<BehaviorModification[]>;
    /**
     * Register new adaptation rule
     */
    registerAdaptationRule(rule: AdaptationRule): void;
    /**
     * Create feedback loop for continuous improvement
     */
    createContinuousImprovementLoop(component: string, metric: string): void;
    /**
     * Implement reinforcement learning feedback loop
     */
    createReinforcementLoop(actionSpace: string[], rewardFunction: (outcome: any) => number): void;
    /**
     * Create exploration-exploitation feedback loop
     */
    createExplorationExploitationLoop(explorationRate?: number): void;
    /**
     * Implement meta-learning feedback loop
     */
    createMetaLearningLoop(): void;
    /**
     * Create adaptive complexity feedback loop
     */
    createComplexityAdaptationLoop(): void;
    /**
     * Apply behavior modification to system parameters
     */
    private applyBehaviorModification;
    /**
     * Learn from feedback patterns to create new adaptation rules
     */
    private learnFromFeedbackPattern;
    /**
     * Initialize default adaptation rules
     */
    private initializeDefaultRules;
    /**
     * Initialize default behavior parameters
     */
    private initializeDefaultParameters;
    /**
     * Update performance metrics based on feedback
     */
    private updatePerformanceMetrics;
    /**
     * Calculate performance score from feedback
     */
    private calculatePerformanceScore;
    /**
     * Get current behavior state
     */
    private getCurrentBehaviorState;
    /**
     * Get metric trend for analysis
     */
    private getMetricTrend;
    /**
     * Check if metric is improving
     */
    private isMetricImproving;
    /**
     * Generate improvement modifications
     */
    private generateImprovementModifications;
    /**
     * Update action probabilities based on reinforcement learning
     */
    private updateActionProbabilities;
    /**
     * Analyze learning effectiveness
     */
    private analyzeLearningEffectiveness;
    /**
     * Adjust learning parameters based on effectiveness
     */
    private adjustLearningParameters;
    /**
     * Get recent performance trend
     */
    private getRecentPerformanceTrend;
    /**
     * Adapt complexity based on performance
     */
    private adaptComplexity;
    /**
     * Update learning curve for component
     */
    private updateLearningCurve;
    /**
     * Detect failure patterns in recent feedback
     */
    private detectFailurePattern;
    /**
     * Detect success patterns in recent feedback
     */
    private detectSuccessPattern;
    /**
     * Create adaptation rule from detected pattern
     */
    private createRuleFromPattern;
    /**
     * Create reinforcement rule from success pattern
     */
    private createReinforcementRule;
    /**
     * Find common elements across contexts
     */
    private findCommonElements;
    /**
     * Get feedback loop statistics
     */
    getStats(): any;
    private getMostActiveComponents;
    private getAdaptationCategories;
}
