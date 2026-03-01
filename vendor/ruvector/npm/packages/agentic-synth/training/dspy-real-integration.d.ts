/**
 * DSPy.ts Real Integration with Agentic-Synth
 *
 * Production-ready integration using actual dspy.ts npm package (v2.1.1)
 * for synthetic data generation optimization and quality improvement.
 *
 * Features:
 * - ChainOfThought reasoning for data quality assessment
 * - BootstrapFewShot optimization for learning from successful generations
 * - Multi-model support (OpenAI, Claude via dspy.ts)
 * - Real-time quality metrics and evaluation
 * - Integration with agentic-synth generators
 *
 * @packageDocumentation
 */
import { EventEmitter } from 'events';
/**
 * DSPy trainer configuration
 */
export interface DSPyTrainerConfig {
    models: string[];
    optimizationRounds?: number;
    minQualityScore?: number;
    maxExamples?: number;
    batchSize?: number;
    evaluationMetrics?: string[];
    enableCaching?: boolean;
    hooks?: {
        onIterationComplete?: (iteration: number, metrics: QualityMetrics) => void;
        onOptimizationComplete?: (result: TrainingResult) => void;
        onError?: (error: Error) => void;
    };
}
/**
 * Quality metrics for generated data
 */
export interface QualityMetrics {
    accuracy: number;
    coherence: number;
    relevance: number;
    diversity: number;
    overallScore: number;
    timestamp: Date;
}
/**
 * Training iteration result
 */
export interface IterationMetrics {
    iteration: number;
    model: string;
    quality: QualityMetrics;
    generatedCount: number;
    duration: number;
    tokenUsage?: number;
}
/**
 * Complete training result
 */
export interface TrainingResult {
    success: boolean;
    iterations: IterationMetrics[];
    bestIteration: IterationMetrics;
    optimizedPrompt: string;
    improvements: {
        initialScore: number;
        finalScore: number;
        improvement: number;
    };
    metadata: {
        totalDuration: number;
        modelsUsed: string[];
        totalGenerated: number;
        convergenceIteration?: number;
    };
}
/**
 * Evaluation result from dspy.ts
 */
export interface EvaluationResult {
    metrics: {
        [key: string]: number;
    };
    passed: number;
    failed: number;
    total: number;
}
/**
 * DSPy example format
 */
export interface DSPyExample {
    input: string;
    output: string;
    quality?: number;
}
/**
 * Main trainer class integrating dspy.ts with agentic-synth
 */
export declare class DSPyAgenticSynthTrainer extends EventEmitter {
    private config;
    private languageModels;
    private chainOfThought?;
    private optimizer?;
    private trainingExamples;
    private currentIteration;
    private bestScore;
    private optimizedPrompt;
    constructor(config: DSPyTrainerConfig);
    /**
     * Initialize DSPy.ts language models and modules
     */
    initialize(): Promise<void>;
    /**
     * Train with optimization using DSPy.ts
     */
    trainWithOptimization(schema: Record<string, any>, examples: DSPyExample[]): Promise<TrainingResult>;
    /**
     * Generate optimized data using trained models
     */
    generateOptimizedData(count: number, schema?: Record<string, any>): Promise<any[]>;
    /**
     * Evaluate data quality using DSPy.ts metrics
     */
    evaluateQuality(data: any[]): Promise<QualityMetrics>;
    /**
     * Run a single training iteration
     */
    private runIteration;
    /**
     * Generate a batch of data samples
     */
    private generateBatch;
    /**
     * Assess data quality for a single item
     */
    private assessDataQuality;
    /**
     * Build generation prompt
     */
    private buildGenerationPrompt;
    /**
     * Parse generated data from model response
     */
    private parseGeneratedData;
    /**
     * Filter successful examples above quality threshold
     */
    private filterSuccessfulExamples;
    /**
     * Update training examples with new results
     */
    private updateTrainingExamples;
    /**
     * Create metric function for DSPy optimizer
     */
    private createMetricFunction;
    /**
     * Convert training examples to DSPy format
     */
    private convertToDSPyExamples;
    /**
     * Calculate simple similarity between two strings
     */
    private calculateSimilarity;
    /**
     * Calculate edit distance between strings
     */
    private editDistance;
    /**
     * Final evaluation across all iterations
     */
    private evaluateFinal;
    /**
     * Calculate average of numbers
     */
    private calculateAverage;
    /**
     * Calculate diversity score
     */
    private calculateDiversity;
    /**
     * Get training statistics
     */
    getStatistics(): {
        totalIterations: number;
        bestScore: number;
        trainingExamples: number;
    };
}
//# sourceMappingURL=dspy-real-integration.d.ts.map