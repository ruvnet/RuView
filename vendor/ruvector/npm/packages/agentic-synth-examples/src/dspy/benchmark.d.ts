/**
 * DSPy.ts Multi-Model Benchmarking System v1.0.0
 *
 * Comprehensive benchmarking suite comparing multiple models across:
 * - Quality metrics (f1Score, exactMatch, bleuScore, rougeScore)
 * - Optimization strategies (BootstrapFewShot, MIPROv2)
 * - Cost-effectiveness analysis
 * - Performance characteristics
 *
 * Real-world implementation using actual dspy.ts v2.1.1 features:
 * - ChainOfThought for reasoning
 * - ReAct for iterative improvement
 * - MultiChainComparison for ensemble decisions
 * - BootstrapFewShot & MIPROv2 optimizers
 *
 * @requires dspy.ts@2.1.1
 * @requires Environment: OPENAI_API_KEY, ANTHROPIC_API_KEY
 */
declare const ChainOfThought: any;
interface ModelConfig {
    name: string;
    provider: 'openai' | 'anthropic' | 'openrouter';
    modelId: string;
    apiKey: string;
    costPer1kTokens: {
        input: number;
        output: number;
    };
    maxTokens: number;
}
interface BenchmarkMetrics {
    quality: {
        f1: number;
        exactMatch: number;
        bleu: number;
        rouge: number;
        overall: number;
    };
    performance: {
        avgLatency: number;
        p50: number;
        p95: number;
        p99: number;
        throughput: number;
        successRate: number;
    };
    cost: {
        totalCost: number;
        costPerSample: number;
        costPerQualityPoint: number;
        inputTokens: number;
        outputTokens: number;
    };
    optimization: {
        baselineQuality: number;
        bootstrapQuality: number;
        miproQuality: number;
        bootstrapImprovement: number;
        miproImprovement: number;
    };
}
interface BenchmarkResult {
    modelName: string;
    timestamp: string;
    metrics: BenchmarkMetrics;
    optimizationHistory: {
        method: 'baseline' | 'bootstrap' | 'mipro';
        round: number;
        quality: number;
        duration: number;
    }[];
    sampleSize: number;
    duration: number;
}
interface ComparisonReport {
    summary: {
        winner: {
            quality: string;
            performance: string;
            cost: string;
            optimization: string;
            overall: string;
        };
        modelsCompared: number;
        totalSamples: number;
        totalDuration: number;
    };
    results: BenchmarkResult[];
    rankings: {
        quality: {
            model: string;
            score: number;
        }[];
        performance: {
            model: string;
            score: number;
        }[];
        cost: {
            model: string;
            score: number;
        }[];
        optimization: {
            model: string;
            score: number;
        }[];
    };
    recommendations: {
        production: string;
        research: string;
        costOptimized: string;
        balanced: string;
    };
}
/**
 * Synthetic Data Generator using Chain of Thought
 */
declare class SyntheticDataModule extends ChainOfThought {
    constructor();
}
export declare class MultiModelBenchmark {
    private models;
    private results;
    private outputDir;
    constructor(outputDir?: string);
    /**
     * Register a model for benchmarking
     */
    addModel(config: ModelConfig): void;
    /**
     * Run comprehensive comparison across all models
     */
    runComparison(sampleSize?: number): Promise<ComparisonReport>;
    /**
     * Benchmark a single model
     */
    private benchmarkModel;
    /**
     * Optimize with BootstrapFewShot
     */
    optimizeWithBootstrap(module: SyntheticDataModule, schema: any, sampleSize: number): Promise<SyntheticDataModule>;
    /**
     * Optimize with MIPROv2
     */
    optimizeWithMIPRO(module: SyntheticDataModule, schema: any, sampleSize: number): Promise<SyntheticDataModule>;
    /**
     * Evaluate module quality
     */
    private evaluateModule;
    /**
     * Measure performance metrics
     */
    private measurePerformance;
    /**
     * Generate training dataset
     */
    private generateTrainingSet;
    /**
     * Generate sample synthetic data
     */
    private generateSampleData;
    /**
     * Calculate quality score for synthetic data
     */
    private calculateQualityScore;
    /**
     * Calculate percentile
     */
    private percentile;
    /**
     * Generate comparison report
     */
    private generateComparisonReport;
    /**
     * Generate and save markdown report
     */
    generateReport(comparison: ComparisonReport): Promise<string>;
}
export { ModelConfig, BenchmarkResult, ComparisonReport, BenchmarkMetrics };
//# sourceMappingURL=benchmark.d.ts.map