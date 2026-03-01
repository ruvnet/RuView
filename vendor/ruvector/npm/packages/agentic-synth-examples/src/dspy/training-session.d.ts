/**
 * DSPy.ts Learning Session - Advanced Multi-Model Training Framework
 *
 * Production-ready implementation for concurrent AI model training with:
 * - DSPy-powered prompt optimization
 * - Multi-model parallel training (Claude, GPT-4, Llama, Gemini)
 * - Automatic quality improvement loops
 * - Real-time metrics and cost tracking
 * - Convergence detection and cross-model learning
 * - Hooks integration for swarm coordination
 *
 * @packageDocumentation
 */
import { EventEmitter } from 'events';
import { z } from 'zod';
/**
 * Supported AI model providers
 */
export declare enum ModelProvider {
    CLAUDE = "claude",
    GPT4 = "gpt4",
    LLAMA = "llama",
    GEMINI = "gemini"
}
/**
 * Training phase states
 */
export declare enum TrainingPhase {
    BASELINE = "baseline",
    OPTIMIZATION = "optimization",
    CROSS_LEARNING = "cross_learning",
    BENCHMARK = "benchmark",
    REPORT = "report"
}
/**
 * Model quality metrics
 */
export interface QualityMetrics {
    score: number;
    accuracy: number;
    coherence: number;
    relevance: number;
    diversity: number;
    creativity: number;
}
/**
 * Model performance metrics
 */
export interface PerformanceMetrics {
    latency: number;
    throughput: number;
    tokensUsed: number;
    cost: number;
    memoryUsage: number;
    errorRate: number;
}
/**
 * Training iteration result
 */
export interface IterationResult {
    iteration: number;
    phase: TrainingPhase;
    modelProvider: ModelProvider;
    quality: QualityMetrics;
    performance: PerformanceMetrics;
    timestamp: Date;
    prompt: string;
    output: string;
    optimizations: string[];
}
/**
 * Model training configuration
 */
export interface ModelConfig {
    provider: ModelProvider;
    model: string;
    apiKey: string;
    temperature?: number;
    maxTokens?: number;
    topP?: number;
    presencePenalty?: number;
    frequencyPenalty?: number;
}
/**
 * DSPy signature for prompt optimization
 */
export interface DSPySignature {
    input: string;
    output: string;
    examples?: Array<{
        input: string;
        output: string;
    }>;
    constraints?: string[];
    objectives?: string[];
}
/**
 * Training session configuration
 */
export interface TrainingConfig {
    models: ModelConfig[];
    optimizationRounds?: number;
    convergenceThreshold?: number;
    maxConcurrency?: number;
    enableCrossLearning?: boolean;
    enableHooksIntegration?: boolean;
    costBudget?: number;
    timeoutPerIteration?: number;
    baselineIterations?: number;
    benchmarkSamples?: number;
}
export declare const TrainingConfigSchema: z.ZodObject<{
    models: z.ZodArray<z.ZodObject<{
        provider: z.ZodNativeEnum<typeof ModelProvider>;
        model: z.ZodString;
        apiKey: z.ZodString;
        temperature: z.ZodOptional<z.ZodNumber>;
        maxTokens: z.ZodOptional<z.ZodNumber>;
        topP: z.ZodOptional<z.ZodNumber>;
        presencePenalty: z.ZodOptional<z.ZodNumber>;
        frequencyPenalty: z.ZodOptional<z.ZodNumber>;
    }, "strip", z.ZodTypeAny, {
        provider: ModelProvider;
        apiKey: string;
        model: string;
        temperature?: number | undefined;
        maxTokens?: number | undefined;
        topP?: number | undefined;
        presencePenalty?: number | undefined;
        frequencyPenalty?: number | undefined;
    }, {
        provider: ModelProvider;
        apiKey: string;
        model: string;
        temperature?: number | undefined;
        maxTokens?: number | undefined;
        topP?: number | undefined;
        presencePenalty?: number | undefined;
        frequencyPenalty?: number | undefined;
    }>, "many">;
    optimizationRounds: z.ZodDefault<z.ZodNumber>;
    convergenceThreshold: z.ZodDefault<z.ZodNumber>;
    maxConcurrency: z.ZodDefault<z.ZodNumber>;
    enableCrossLearning: z.ZodDefault<z.ZodBoolean>;
    enableHooksIntegration: z.ZodDefault<z.ZodBoolean>;
    costBudget: z.ZodOptional<z.ZodNumber>;
    timeoutPerIteration: z.ZodDefault<z.ZodNumber>;
    baselineIterations: z.ZodDefault<z.ZodNumber>;
    benchmarkSamples: z.ZodDefault<z.ZodNumber>;
}, "strip", z.ZodTypeAny, {
    maxConcurrency: number;
    models: {
        provider: ModelProvider;
        apiKey: string;
        model: string;
        temperature?: number | undefined;
        maxTokens?: number | undefined;
        topP?: number | undefined;
        presencePenalty?: number | undefined;
        frequencyPenalty?: number | undefined;
    }[];
    optimizationRounds: number;
    convergenceThreshold: number;
    enableCrossLearning: boolean;
    enableHooksIntegration: boolean;
    timeoutPerIteration: number;
    baselineIterations: number;
    benchmarkSamples: number;
    costBudget?: number | undefined;
}, {
    models: {
        provider: ModelProvider;
        apiKey: string;
        model: string;
        temperature?: number | undefined;
        maxTokens?: number | undefined;
        topP?: number | undefined;
        presencePenalty?: number | undefined;
        frequencyPenalty?: number | undefined;
    }[];
    maxConcurrency?: number | undefined;
    optimizationRounds?: number | undefined;
    convergenceThreshold?: number | undefined;
    enableCrossLearning?: boolean | undefined;
    enableHooksIntegration?: boolean | undefined;
    costBudget?: number | undefined;
    timeoutPerIteration?: number | undefined;
    baselineIterations?: number | undefined;
    benchmarkSamples?: number | undefined;
}>;
/**
 * Abstract base class for all model-specific training agents
 */
export declare abstract class ModelTrainingAgent extends EventEmitter {
    protected config: ModelConfig;
    protected results: IterationResult[];
    protected currentIteration: number;
    protected totalCost: number;
    protected isConverged: boolean;
    constructor(config: ModelConfig);
    /**
     * Execute a single training iteration
     */
    abstract execute(prompt: string, signature: DSPySignature): Promise<IterationResult>;
    /**
     * Calculate quality metrics for generated output
     */
    protected calculateQuality(output: string, expectedSignature: DSPySignature): Promise<QualityMetrics>;
    /**
     * Calculate performance metrics
     */
    protected calculatePerformance(startTime: number, endTime: number, tokensUsed: number): PerformanceMetrics;
    /**
     * Calculate cost based on tokens used
     */
    protected calculateCost(tokensUsed: number): number;
    /**
     * Get cost per 1K tokens for this model
     */
    protected abstract getCostPer1KTokens(): number;
    /**
     * Get current results
     */
    getResults(): IterationResult[];
    /**
     * Get total cost
     */
    getTotalCost(): number;
    /**
     * Check if converged
     */
    hasConverged(): boolean;
    /**
     * Calculate overall quality score
     */
    private calculateOverallScore;
    private calculateAccuracy;
    private calculateCoherence;
    private calculateRelevance;
    private calculateDiversity;
    private calculateCreativity;
    private checkConstraint;
    private calculateErrorRate;
}
/**
 * Claude Sonnet training agent
 */
export declare class ClaudeSonnetAgent extends ModelTrainingAgent {
    execute(prompt: string, signature: DSPySignature): Promise<IterationResult>;
    private callClaudeAPI;
    private estimateTokens;
    protected getCostPer1KTokens(): number;
}
/**
 * GPT-4 training agent
 */
export declare class GPT4Agent extends ModelTrainingAgent {
    execute(prompt: string, signature: DSPySignature): Promise<IterationResult>;
    private callGPT4API;
    private estimateTokens;
    protected getCostPer1KTokens(): number;
}
/**
 * Llama training agent
 */
export declare class LlamaAgent extends ModelTrainingAgent {
    execute(prompt: string, signature: DSPySignature): Promise<IterationResult>;
    private callLlamaAPI;
    private estimateTokens;
    protected getCostPer1KTokens(): number;
}
/**
 * Gemini training agent
 */
export declare class GeminiAgent extends ModelTrainingAgent {
    execute(prompt: string, signature: DSPySignature): Promise<IterationResult>;
    private callGeminiAPI;
    private estimateTokens;
    protected getCostPer1KTokens(): number;
}
/**
 * Collects and aggregates metrics across all training iterations
 */
export declare class BenchmarkCollector {
    private metrics;
    /**
     * Add result to collection
     */
    addResult(result: IterationResult): void;
    /**
     * Get metrics for specific model
     */
    getModelMetrics(provider: ModelProvider): IterationResult[];
    /**
     * Calculate aggregate statistics
     */
    getAggregateStats(provider: ModelProvider): {
        provider: ModelProvider;
        totalIterations: number;
        avgQualityScore: number;
        minQualityScore: number;
        maxQualityScore: number;
        avgLatency: number;
        minLatency: number;
        maxLatency: number;
        totalCost: number;
        avgCostPer1K: number;
        convergenceRate: number;
        improvementRate: number;
    } | null;
    /**
     * Get comparison across all models
     */
    getComparison(): Record<string, any>;
    /**
     * Get best performing model
     */
    getBestModel(): ModelProvider | null;
    /**
     * Generate detailed report
     */
    generateReport(): string;
    private average;
    private calculateConvergenceRate;
    private calculateImprovementRate;
}
/**
 * DSPy-powered prompt optimization engine
 */
export declare class OptimizationEngine {
    private signatures;
    private optimizationHistory;
    /**
     * Create a new DSPy signature
     */
    createSignature(name: string, input: string, output: string, options?: {
        examples?: Array<{
            input: string;
            output: string;
        }>;
        constraints?: string[];
        objectives?: string[];
    }): DSPySignature;
    /**
     * Optimize prompt based on previous results
     */
    optimizePrompt(basePrompt: string, results: IterationResult[], signature: DSPySignature): Promise<string>;
    /**
     * Enable cross-model learning
     */
    crossModelOptimization(allResults: Map<ModelProvider, IterationResult[]>): Promise<Map<ModelProvider, string>>;
    private addExamples;
    private addConstraints;
    private addObjectives;
    private incorporateBestPractices;
    private extractCommonPhrases;
    private mergePromptStrategies;
}
/**
 * Main DSPy training session orchestrator
 */
export declare class DSPyTrainingSession extends EventEmitter {
    private config;
    private agents;
    private collector;
    private optimizer;
    private currentPhase;
    private startTime;
    private totalCost;
    constructor(config: TrainingConfig);
    /**
     * Initialize model agents
     */
    private initializeAgents;
    /**
     * Run complete training pipeline
     */
    run(basePrompt: string, signature: DSPySignature): Promise<void>;
    /**
     * Phase 1: Baseline generation (all models)
     */
    private runBaseline;
    /**
     * Phase 2: DSPy optimization (5 rounds per model)
     */
    private runOptimization;
    /**
     * Phase 3: Cross-model learning (share best patterns)
     */
    private runCrossLearning;
    /**
     * Phase 4: Final benchmark comparison
     */
    private runBenchmark;
    /**
     * Phase 5: Generate comprehensive report
     */
    private generateReport;
    /**
     * Handle iteration results
     */
    private handleIteration;
    /**
     * Integrate with Claude Flow hooks for swarm coordination
     */
    private integrateWithHooks;
    /**
     * Get current session statistics
     */
    getStatistics(): {
        currentPhase: TrainingPhase;
        totalCost: number;
        duration: number;
        bestModel: ModelProvider | null;
        comparison: Record<string, any>;
    };
    /**
     * Stop training session
     */
    stop(): void;
}
export type { QualityMetrics, PerformanceMetrics, IterationResult, ModelConfig, DSPySignature, TrainingConfig };
//# sourceMappingURL=training-session.d.ts.map