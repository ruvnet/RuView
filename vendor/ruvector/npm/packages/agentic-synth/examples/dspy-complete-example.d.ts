/**
 * COMPREHENSIVE DSPy.ts + AgenticSynth Integration Example
 *
 * E-commerce Product Data Generation with DSPy Optimization
 *
 * This example demonstrates:
 * 1. ✅ Real DSPy.ts (v2.1.1) module usage - ChainOfThought, Predict, Refine
 * 2. ✅ Integration with AgenticSynth for baseline data generation
 * 3. ✅ BootstrapFewShot optimizer for learning from high-quality examples
 * 4. ✅ Quality metrics and comparison (baseline vs optimized)
 * 5. ✅ Production-ready error handling and progress tracking
 * 6. ✅ Multiple LM provider support (OpenAI, Anthropic)
 *
 * Usage:
 * ```bash
 * export OPENAI_API_KEY=sk-...
 * export GEMINI_API_KEY=...
 * npx tsx examples/dspy-complete-example.ts
 * ```
 *
 * @author rUv
 * @license MIT
 */
import 'dotenv/config';
interface Product {
    id?: string;
    name: string;
    category: string;
    description: string;
    price: number;
    rating: number;
    features?: string[];
    tags?: string[];
}
interface QualityMetrics {
    completeness: number;
    coherence: number;
    persuasiveness: number;
    seoQuality: number;
    overall: number;
}
interface ComparisonResults {
    baseline: {
        products: Product[];
        avgQuality: number;
        metrics: QualityMetrics;
        generationTime: number;
        cost: number;
    };
    optimized: {
        products: Product[];
        avgQuality: number;
        metrics: QualityMetrics;
        generationTime: number;
        cost: number;
    };
    improvement: {
        qualityGain: number;
        speedChange: number;
        costEfficiency: number;
    };
}
/**
 * Calculate quality metrics for a product description
 */
declare function calculateQualityMetrics(product: Product): QualityMetrics;
/**
 * Calculate average quality across multiple products
 */
declare function calculateAverageQuality(products: Product[]): {
    avgQuality: number;
    metrics: QualityMetrics;
};
/**
 * Generate baseline product data using AgenticSynth (Gemini)
 */
declare function generateBaseline(count: number): Promise<{
    products: Product[];
    time: number;
    cost: number;
}>;
/**
 * Create high-quality training examples for DSPy
 */
declare function createTrainingExamples(): Array<{
    category: string;
    priceRange: string;
    product: Product;
}>;
/**
 * Setup DSPy with OpenAI and create optimized module
 */
declare function setupDSPyOptimization(): Promise<{
    optimizedModule: any;
    setupTime: number;
}>;
/**
 * Generate products using optimized DSPy module
 */
declare function generateWithDSPy(optimizedModule: any, count: number): Promise<{
    products: Product[];
    time: number;
    cost: number;
}>;
/**
 * Compare baseline vs optimized results
 */
declare function compareResults(baselineData: {
    products: Product[];
    time: number;
    cost: number;
}, optimizedData: {
    products: Product[];
    time: number;
    cost: number;
}): ComparisonResults;
export { generateBaseline, setupDSPyOptimization, generateWithDSPy, compareResults, calculateQualityMetrics, calculateAverageQuality, createTrainingExamples };
//# sourceMappingURL=dspy-complete-example.d.ts.map