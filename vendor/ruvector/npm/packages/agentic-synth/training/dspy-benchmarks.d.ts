/**
 * DSPy Benchmark Comparison Framework
 *
 * Comprehensive benchmarking suite for comparing multiple models across
 * quality, performance, cost, learning, and diversity metrics.
 *
 * Features:
 * - Multi-model comparison with statistical significance
 * - Scalability testing (100 to 100K samples)
 * - Cost-effectiveness analysis
 * - Quality convergence tracking
 * - Diversity analysis
 * - Pareto frontier optimization
 * - Use case recommendations
 */
interface ModelConfig {
    name: string;
    provider: 'openrouter' | 'gemini' | 'anthropic' | 'openai';
    model: string;
    costPer1kTokens: number;
    maxTokens: number;
    apiKey?: string;
}
interface QualityMetrics {
    accuracy: number;
    coherence: number;
    validity: number;
    consistency: number;
    completeness: number;
    overall: number;
}
interface PerformanceMetrics {
    latencyP50: number;
    latencyP95: number;
    latencyP99: number;
    avgLatency: number;
    minLatency: number;
    maxLatency: number;
    throughput: number;
    successRate: number;
}
interface CostMetrics {
    totalCost: number;
    costPerSample: number;
    costPerQualityPoint: number;
    tokensUsed: number;
    efficiency: number;
}
interface LearningMetrics {
    improvementRate: number;
    convergenceSpeed: number;
    learningCurve: number[];
    plateauGeneration: number;
    finalQuality: number;
}
interface DiversityMetrics {
    uniqueValues: number;
    patternVariety: number;
    distributionEntropy: number;
    coverageScore: number;
    noveltyRate: number;
}
interface BenchmarkResult {
    modelName: string;
    sampleSize: number;
    quality: QualityMetrics;
    performance: PerformanceMetrics;
    cost: CostMetrics;
    learning: LearningMetrics;
    diversity: DiversityMetrics;
    timestamp: string;
    duration: number;
}
interface ComparisonResult {
    models: string[];
    winner: {
        overall: string;
        quality: string;
        performance: string;
        cost: string;
        learning: string;
        diversity: string;
    };
    statisticalSignificance: {
        [key: string]: number;
    };
    paretoFrontier: string[];
    recommendations: {
        [useCase: string]: string;
    };
}
interface ScalabilityResult {
    modelName: string;
    sampleSizes: number[];
    latencies: number[];
    throughputs: number[];
    costs: number[];
    qualities: number[];
    scalingEfficiency: number;
}
declare class StatisticalAnalyzer {
    /**
     * Calculate mean of array
     */
    static mean(values: number[]): number;
    /**
     * Calculate standard deviation
     */
    static stdDev(values: number[]): number;
    /**
     * Calculate percentile
     */
    static percentile(values: number[], p: number): number;
    /**
     * Perform t-test to determine statistical significance
     * Returns p-value
     */
    static tTest(sample1: number[], sample2: number[]): number;
    /**
     * Simplified t-distribution CDF approximation
     */
    private static tDistribution;
    /**
     * Calculate Shannon entropy for diversity measurement
     */
    static entropy(values: any[]): number;
}
export declare class BenchmarkSuite {
    private models;
    private outputDir;
    private results;
    constructor(outputDir?: string);
    /**
     * Add a model configuration to the benchmark suite
     */
    addModel(config: ModelConfig): void;
    /**
     * Add multiple common models for quick testing
     */
    addCommonModels(): void;
    /**
     * Run comprehensive comparison across all models
     */
    runModelComparison(sampleSize?: number): Promise<ComparisonResult>;
    /**
     * Test scalability from 100 to 100K samples
     */
    runScalabilityTest(): Promise<ScalabilityResult[]>;
    /**
     * Analyze cost-effectiveness across models
     */
    runCostAnalysis(): Promise<void>;
    /**
     * Measure quality convergence and learning rates
     */
    runQualityConvergence(generations?: number): Promise<void>;
    /**
     * Analyze data diversity and variety
     */
    runDiversityAnalysis(sampleSize?: number): Promise<void>;
    /**
     * Benchmark a single model
     */
    private benchmarkModel;
    /**
     * Calculate quality metrics
     */
    private calculateQualityMetrics;
    /**
     * Calculate performance metrics
     */
    private calculatePerformanceMetrics;
    /**
     * Calculate cost metrics
     */
    private calculateCostMetrics;
    /**
     * Calculate learning metrics
     */
    private calculateLearningMetrics;
    /**
     * Calculate diversity metrics
     */
    private calculateDiversityMetrics;
    /**
     * Compare results and generate comparison report
     */
    private compareResults;
    /**
     * Calculate Pareto frontier for quality vs cost trade-off
     */
    private calculateParetoFrontier;
    /**
     * Find generation where quality plateaus
     */
    private findPlateauGeneration;
    /**
     * Generate comprehensive JSON report
     */
    generateJSONReport(comparison: ComparisonResult): Promise<void>;
    /**
     * Generate comprehensive Markdown report
     */
    generateMarkdownReport(comparison: ComparisonResult): Promise<void>;
    /**
     * Build markdown report content
     */
    private buildMarkdownReport;
    /**
     * Generate summary statistics
     */
    private generateSummary;
    /**
     * Generate conclusion for report
     */
    private generateConclusion;
    /**
     * Save scalability results
     */
    private saveScalabilityResults;
    /**
     * Save convergence data
     */
    private saveConvergenceData;
}
export { ModelConfig, BenchmarkResult, ComparisonResult, ScalabilityResult, StatisticalAnalyzer };
//# sourceMappingURL=dspy-benchmarks.d.ts.map