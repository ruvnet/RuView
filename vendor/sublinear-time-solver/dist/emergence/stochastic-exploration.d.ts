/**
 * Stochastic Exploration System
 * Generates unpredictable outputs through controlled randomness and exploration
 */
export interface ExplorationResult {
    output: any;
    novelty: number;
    confidence: number;
    explorationPath: string[];
    surpriseLevel: number;
}
export interface ExplorationSpace {
    dimensions: string[];
    bounds: {
        [key: string]: [number, number];
    };
    constraints: any[];
}
export declare class StochasticExplorationEngine {
    private explorationHistory;
    private currentTemperature;
    private coolingRate;
    private minTemperature;
    private explorationBudget;
    /**
     * Generate unpredictable outputs using stochastic sampling
     */
    exploreUnpredictably(input: any, tools: any[]): Promise<ExplorationResult>;
    /**
     * Generate multiple diverse explorations
     */
    generateDiverseExplorations(input: any, tools: any[], count?: number): Promise<ExplorationResult[]>;
    /**
     * Adaptive exploration based on success/failure feedback
     */
    adaptExploration(feedback: {
        success: boolean;
        utility: number;
        feedback: string;
    }): void;
    /**
     * Define multi-dimensional exploration spaces
     */
    private defineExplorationSpaces;
    /**
     * Stochastic sampling using temperature-controlled exploration
     */
    private stochasticSampling;
    /**
     * Temperature-controlled sampling
     */
    private temperatureSample;
    /**
     * Convert numeric values to exploration actions
     */
    private valueToAction;
    /**
     * Generate completely random action
     */
    private generateRandomAction;
    /**
     * Execute exploration path
     */
    private executePath;
    /**
     * Execute individual exploration action
     */
    private executeAction;
    /**
     * Calculate novelty compared to exploration history
     */
    private calculateNovelty;
    /**
     * Calculate surprise level
     */
    private calculateSurprise;
    /**
     * Calculate confidence in result
     */
    private calculateConfidence;
    /**
     * Update exploration temperature (simulated annealing)
     */
    private updateTemperature;
    /**
     * Penalize similar results to encourage diversity
     */
    private penalizeSimilarity;
    private applyTool;
    private applyCreativeTransform;
    private applyDeepReasoning;
    private reverseInput;
    private combineUnexpected;
    private crossDomainLeap;
    private defaultAction;
    private calculateSimilarity;
    private measureComplexity;
    private measureRandomness;
    private summarizeResult;
    private generateAlternativeResult;
    private randomizeParameters;
    private highCreativityTransform;
    private mediumCreativityTransform;
    private reasoningStep;
    private generateMetaphor;
    private generateAbstraction;
    private generateAnalogy;
    /**
     * Get exploration statistics
     */
    getExplorationStats(): any;
    private calculateAverageNovelty;
    private calculateAverageSurprise;
    private calculateRecentSuccess;
}
