/**
 * Self-Improving Data Generation with Feedback Loops
 *
 * This example demonstrates:
 * - Quality scoring and regeneration
 * - A/B testing data for model improvement
 * - Pattern learning from production data
 * - Adaptive schema evolution
 */
import type { GenerationResult } from '../../src/types.js';
/**
 * Generate data with quality scores and regenerate low-quality samples
 */
export declare function qualityScoringLoop(): Promise<void>;
/**
 * Generate A/B test data to improve model performance
 */
export declare function abTestingData(): Promise<GenerationResult<unknown>>;
/**
 * Learn patterns from production data and generate similar synthetic data
 */
export declare function patternLearningLoop(): Promise<GenerationResult<unknown>>;
/**
 * Evolve data schema based on feedback and changing requirements
 */
export declare function adaptiveSchemaEvolution(): Promise<{
    v1: GenerationResult<unknown>;
    v2: GenerationResult<unknown>;
    v3: GenerationResult<unknown>;
}>;
/**
 * Generate data for active learning - focus on uncertain/informative samples
 */
export declare function activeLearningData(): Promise<GenerationResult<unknown>>;
/**
 * Generate evaluation data for continuous model monitoring
 */
export declare function continuousEvaluationData(): Promise<GenerationResult<unknown>>;
export declare function runAllFeedbackLoopExamples(): Promise<void>;
//# sourceMappingURL=feedback-loop.d.ts.map