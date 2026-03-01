/**
 * Model Comparison Benchmark
 *
 * Head-to-head comparison between:
 * - Qwen2.5-0.5B-Instruct (base model)
 * - RuvLTRA Claude Code 0.5B (fine-tuned for Claude Code)
 *
 * Tests routing accuracy and embedding quality for Claude Code use cases.
 */
import { type RoutingBenchmarkResults } from './routing-benchmark';
import { type EmbeddingBenchmarkResults } from './embedding-benchmark';
/** Model configuration */
export interface ModelConfig {
    id: string;
    name: string;
    url: string;
    filename: string;
    sizeBytes: number;
    description: string;
}
/** Comparison models */
export declare const COMPARISON_MODELS: Record<string, ModelConfig>;
/** Comparison result */
export interface ComparisonResult {
    modelId: string;
    modelName: string;
    routing: RoutingBenchmarkResults;
    embedding: EmbeddingBenchmarkResults;
    overallScore: number;
}
/** Full comparison results */
export interface FullComparisonResults {
    timestamp: string;
    baseline: ComparisonResult;
    models: ComparisonResult[];
    winner: string;
    summary: string;
}
/**
 * Get models directory
 */
export declare function getModelsDir(): string;
/**
 * Check if model is downloaded
 */
export declare function isModelDownloaded(modelId: string): boolean;
/**
 * Download a model with progress
 */
export declare function downloadModel(modelId: string, onProgress?: (percent: number, speed: number) => void): Promise<string>;
/**
 * Run comparison for a single model
 */
export declare function runModelComparison(modelId: string, modelName: string, embedder: (text: string) => number[]): ComparisonResult;
/**
 * Format comparison results
 */
export declare function formatComparisonResults(results: FullComparisonResults): string;
/**
 * Run full comparison
 */
export declare function runFullComparison(): Promise<FullComparisonResults>;
declare const _default: {
    COMPARISON_MODELS: Record<string, ModelConfig>;
    runFullComparison: typeof runFullComparison;
    formatComparisonResults: typeof formatComparisonResults;
    downloadModel: typeof downloadModel;
    isModelDownloaded: typeof isModelDownloaded;
};
export default _default;
//# sourceMappingURL=model-comparison.d.ts.map