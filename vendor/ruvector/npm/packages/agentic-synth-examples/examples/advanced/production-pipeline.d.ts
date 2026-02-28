/**
 * ADVANCED TUTORIAL: Production Pipeline
 *
 * Build a complete production-ready data generation pipeline with:
 * - Error handling and retry logic
 * - Monitoring and metrics
 * - Rate limiting and cost controls
 * - Batch processing and caching
 * - Quality validation
 *
 * What you'll learn:
 * - Production-grade error handling
 * - Performance monitoring
 * - Cost optimization
 * - Scalability patterns
 * - Deployment best practices
 *
 * Prerequisites:
 * - Complete previous tutorials
 * - Set GEMINI_API_KEY environment variable
 * - npm install @ruvector/agentic-synth
 *
 * Run: npx tsx examples/advanced/production-pipeline.ts
 */
import { GenerationResult } from '@ruvector/agentic-synth';
interface PipelineConfig {
    maxRetries: number;
    retryDelay: number;
    batchSize: number;
    maxConcurrency: number;
    qualityThreshold: number;
    costBudget: number;
    rateLimitPerMinute: number;
    enableCaching: boolean;
    outputDirectory: string;
}
interface PipelineMetrics {
    totalRequests: number;
    successfulRequests: number;
    failedRequests: number;
    totalDuration: number;
    totalCost: number;
    averageQuality: number;
    cacheHits: number;
    retries: number;
    errors: Array<{
        timestamp: Date;
        error: string;
        context: any;
    }>;
}
interface QualityValidator {
    validate(data: any): {
        valid: boolean;
        score: number;
        issues: string[];
    };
}
declare class ProductionPipeline {
    private config;
    private synth;
    private metrics;
    private requestsThisMinute;
    private minuteStartTime;
    constructor(config?: Partial<PipelineConfig>);
    private checkRateLimit;
    private checkCostBudget;
    private generateWithRetry;
    private processBatch;
    run(requests: any[], validator?: QualityValidator): Promise<GenerationResult[]>;
    private saveResults;
    private displayMetrics;
    getMetrics(): PipelineMetrics;
}
declare class ProductQualityValidator implements QualityValidator {
    validate(data: any[]): {
        valid: boolean;
        score: number;
        issues: string[];
    };
}
export { ProductionPipeline, ProductQualityValidator, PipelineConfig, PipelineMetrics };
//# sourceMappingURL=production-pipeline.d.ts.map