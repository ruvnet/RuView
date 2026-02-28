/**
 * INTERMEDIATE TUTORIAL: Multi-Model Comparison
 *
 * Compare multiple AI models (Gemini, Claude, GPT-4) to find the best
 * performer for your specific task. Includes benchmarking, cost tracking,
 * and performance metrics.
 *
 * What you'll learn:
 * - Running parallel model comparisons
 * - Benchmarking quality and speed
 * - Tracking costs per model
 * - Selecting the best model for production
 *
 * Prerequisites:
 * - Set API keys: GEMINI_API_KEY, ANTHROPIC_API_KEY, OPENAI_API_KEY
 * - npm install dspy.ts @ruvector/agentic-synth
 *
 * Run: npx tsx examples/intermediate/multi-model-comparison.ts
 */
import { Prediction } from 'dspy.ts';
interface ModelConfig {
    name: string;
    provider: string;
    model: string;
    apiKey: string;
    costPer1kTokens: number;
    capabilities: string[];
}
declare const models: ModelConfig[];
interface BenchmarkResult {
    modelName: string;
    qualityScore: number;
    avgResponseTime: number;
    estimatedCost: number;
    successRate: number;
    outputs: Prediction[];
    errors: string[];
}
declare function benchmarkModel(config: ModelConfig): Promise<BenchmarkResult>;
declare function runComparison(): Promise<BenchmarkResult[]>;
export { runComparison, benchmarkModel, models };
//# sourceMappingURL=multi-model-comparison.d.ts.map