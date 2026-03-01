/**
 * Ad Optimization Simulator
 *
 * Generates optimization scenario data including:
 * - Budget allocation simulations
 * - Bid strategy testing data
 * - Audience segmentation data
 * - Creative performance variations
 * - ROAS optimization scenarios
 */
declare function simulateBudgetAllocation(): Promise<import("../../src/types.js").GenerationResult<unknown>>;
declare function simulateBidStrategies(): Promise<import("../../src/types.js").GenerationResult<unknown>>;
declare function simulateAudienceSegmentation(): Promise<import("../../src/types.js").GenerationResult<unknown>>;
declare function simulateCreativePerformance(): Promise<import("../../src/types.js").GenerationResult<unknown>>;
declare function simulateROASOptimization(): Promise<import("../../src/types.js").GenerationResult<unknown>>;
declare function simulateOptimizationImpact(): Promise<import("../../src/types.js").GenerationResult<unknown>>;
declare function simulateMultiVariateTesting(): Promise<import("../../src/types.js").GenerationResult<unknown>>;
declare function simulateDaypartingOptimization(): Promise<import("../../src/types.js").GenerationResult<unknown>>;
declare function simulateGeoTargetingOptimization(): Promise<import("../../src/types.js").GenerationResult<unknown>>;
declare function simulateBatchOptimization(): Promise<import("../../src/types.js").GenerationResult<unknown>[]>;
export declare function runOptimizationExamples(): Promise<void>;
export { simulateBudgetAllocation, simulateBidStrategies, simulateAudienceSegmentation, simulateCreativePerformance, simulateROASOptimization, simulateOptimizationImpact, simulateMultiVariateTesting, simulateDaypartingOptimization, simulateGeoTargetingOptimization, simulateBatchOptimization };
//# sourceMappingURL=optimization-simulator.d.ts.map