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

import { AgenticSynth, GenerationResult } from '@ruvector/agentic-synth';
import { writeFileSync, existsSync, mkdirSync } from 'fs';
import { join } from 'path';

// Pipeline configuration
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

// Metrics tracking
interface PipelineMetrics {
  totalRequests: number;
  successfulRequests: number;
  failedRequests: number;
  totalDuration: number;
  totalCost: number;
  averageQuality: number;
  cacheHits: number;
  retries: number;
  errors: Array<{ timestamp: Date; error: string; context: any }>;
}

// Quality validator
interface QualityValidator {
  validate(data: any): { valid: boolean; score: number; issues: string[] };
}

// Production-grade pipeline
class ProductionPipeline {
  private config: PipelineConfig;
  private synth: AgenticSynth;
  private metrics: PipelineMetrics;
  private requestsThisMinute: number = 0;
  private minuteStartTime: number = Date.now();

  constructor(config: Partial<PipelineConfig> = {}) {
    this.config = {
      maxRetries: config.maxRetries || 3,
      retryDelay: config.retryDelay || 1000,
      batchSize: config.batchSize || 10,
      maxConcurrency: config.maxConcurrency || 3,
      qualityThreshold: config.qualityThreshold || 0.7,
      costBudget: config.costBudget || 10.0,
      rateLimitPerMinute: config.rateLimitPerMinute || 60,
      enableCaching: config.enableCaching !== false,
      outputDirectory: config.outputDirectory || './output'
    };

    this.synth = new AgenticSynth({
      provider: 'gemini',
      apiKey: process.env.GEMINI_API_KEY,
      model: 'gemini-2.0-flash-exp',
      cacheStrategy: this.config.enableCaching ? 'memory' : 'none',
      cacheTTL: 3600,
      maxRetries: this.config.maxRetries,
      timeout: 30000
    });

    this.metrics = {
      totalRequests: 0,
      successfulRequests: 0,
      failedRequests: 0,
      totalDuration: 0,
      totalCost: 0,
      averageQuality: 0,
      cacheHits: 0,
      retries: 0,
      errors: []
    };

    // Ensure output directory exists
    if (!existsSync(this.config.outputDirectory)) {
      mkdirSync(this.config.outputDirectory, { recursive: true });
    }
  }

  // Rate limiting check
  private async checkRateLimit(): Promise<void> {
    const now = Date.now();
    const elapsedMinutes = (now - this.minuteStartTime) / 60000;

    if (elapsedMinutes >= 1) {
      // Reset counter for new minute
      this.requestsThisMinute = 0;
      this.minuteStartTime = now;
    }

    if (this.requestsThisMinute >= this.config.rateLimitPerMinute) {
      const waitTime = 60000 - (now - this.minuteStartTime);
      console.log(`⏳ Rate limit reached, waiting ${Math.ceil(waitTime / 1000)}s...`);
      await new Promise(resolve => setTimeout(resolve, waitTime));
      this.requestsThisMinute = 0;
      this.minuteStartTime = Date.now();
    }
  }

  // Cost check
  private checkCostBudget(): void {
    if (this.metrics.totalCost >= this.config.costBudget) {
      throw new Error(`Cost budget exceeded: $${this.metrics.totalCost.toFixed(4)} >= $${this.config.costBudget}`);
    }
  }

  // Generate with retry logic
  private async generateWithRetry(
    options: any,
    attempt: number = 1
  ): Promise<GenerationResult> {
    try {
      await this.checkRateLimit();
      this.checkCostBudget();

      this.requestsThisMinute++;
      this.metrics.totalRequests++;

      const startTime = Date.now();
      const result = await this.synth.generateStructured(options);
      const duration = Date.now() - startTime;

      this.metrics.totalDuration += duration;
      this.metrics.successfulRequests++;

      if (result.metadata.cached) {
        this.metrics.cacheHits++;
      }

      // Estimate cost (rough approximation)
      const estimatedCost = result.metadata.cached ? 0 : 0.0001;
      this.metrics.totalCost += estimatedCost;

      return result;

    } catch (error) {
      const errorMsg = error instanceof Error ? error.message : 'Unknown error';

      if (attempt < this.config.maxRetries) {
        this.metrics.retries++;
        console.log(`⚠️  Attempt ${attempt} failed, retrying... (${errorMsg})`);

        await new Promise(resolve =>
          setTimeout(resolve, this.config.retryDelay * attempt)
        );

        return this.generateWithRetry(options, attempt + 1);
      } else {
        this.metrics.failedRequests++;
        this.metrics.errors.push({
          timestamp: new Date(),
          error: errorMsg,
          context: options
        });
        throw error;
      }
    }
  }

  // Process a single batch
  private async processBatch(
    requests: any[],
    validator?: QualityValidator
  ): Promise<GenerationResult[]> {
    const results: GenerationResult[] = [];

    // Process with concurrency control
    for (let i = 0; i < requests.length; i += this.config.maxConcurrency) {
      const batch = requests.slice(i, i + this.config.maxConcurrency);

      const batchResults = await Promise.allSettled(
        batch.map(req => this.generateWithRetry(req))
      );

      batchResults.forEach((result, idx) => {
        if (result.status === 'fulfilled') {
          const genResult = result.value;

          // Validate quality if validator provided
          if (validator) {
            const validation = validator.validate(genResult.data);

            if (validation.valid) {
              results.push(genResult);
            } else {
              console.log(`⚠️  Quality validation failed (score: ${validation.score.toFixed(2)})`);
              console.log(`   Issues: ${validation.issues.join(', ')}`);
            }
          } else {
            results.push(genResult);
          }
        } else {
          console.error(`❌ Batch item ${i + idx} failed:`, result.reason);
        }
      });
    }

    return results;
  }

  // Main pipeline execution
  async run(
    requests: any[],
    validator?: QualityValidator
  ): Promise<GenerationResult[]> {
    console.log('🏭 Starting Production Pipeline\n');
    console.log('=' .repeat(70));
    console.log(`\nConfiguration:`);
    console.log(`  Total Requests: ${requests.length}`);
    console.log(`  Batch Size: ${this.config.batchSize}`);
    console.log(`  Max Concurrency: ${this.config.maxConcurrency}`);
    console.log(`  Max Retries: ${this.config.maxRetries}`);
    console.log(`  Cost Budget: $${this.config.costBudget}`);
    console.log(`  Rate Limit: ${this.config.rateLimitPerMinute}/min`);
    console.log(`  Caching: ${this.config.enableCaching ? 'Enabled' : 'Disabled'}`);
    console.log(`  Output: ${this.config.outputDirectory}`);
    console.log('\n' + '=' .repeat(70) + '\n');

    const startTime = Date.now();
    const allResults: GenerationResult[] = [];

    // Split into batches
    const batches = [];
    for (let i = 0; i < requests.length; i += this.config.batchSize) {
      batches.push(requests.slice(i, i + this.config.batchSize));
    }

    console.log(`📦 Processing ${batches.length} batches...\n`);

    // Process each batch
    for (let i = 0; i < batches.length; i++) {
      console.log(`\nBatch ${i + 1}/${batches.length} (${batches[i].length} items)`);
      console.log('─'.repeat(70));

      try {
        const batchResults = await this.processBatch(batches[i], validator);
        allResults.push(...batchResults);

        console.log(`✓ Batch complete: ${batchResults.length}/${batches[i].length} successful`);
        console.log(`  Cost so far: $${this.metrics.totalCost.toFixed(4)}`);
        console.log(`  Cache hits: ${this.metrics.cacheHits}`);

      } catch (error) {
        console.error(`✗ Batch failed:`, error instanceof Error ? error.message : 'Unknown error');

        if (error instanceof Error && error.message.includes('budget')) {
          console.log('\n⚠️  Cost budget exceeded, stopping pipeline...');
          break;
        }
      }
    }

    const totalTime = Date.now() - startTime;

    // Save results
    await this.saveResults(allResults);

    // Display metrics
    this.displayMetrics(totalTime);

    return allResults;
  }

  // Save results to disk
  private async saveResults(results: GenerationResult[]): Promise<void> {
    try {
      const timestamp = new Date().toISOString().replace(/[:.]/g, '-');
      const filename = `generation-${timestamp}.json`;
      const filepath = join(this.config.outputDirectory, filename);

      const output = {
        timestamp: new Date(),
        results: results.map(r => r.data),
        metadata: {
          count: results.length,
          metrics: this.metrics
        }
      };

      writeFileSync(filepath, JSON.stringify(output, null, 2));
      console.log(`\n💾 Results saved to: ${filepath}`);

      // Save metrics separately
      const metricsFile = join(this.config.outputDirectory, `metrics-${timestamp}.json`);
      writeFileSync(metricsFile, JSON.stringify(this.metrics, null, 2));
      console.log(`📊 Metrics saved to: ${metricsFile}`);

    } catch (error) {
      console.error('⚠️  Failed to save results:', error instanceof Error ? error.message : 'Unknown error');
    }
  }

  // Display comprehensive metrics
  private displayMetrics(totalTime: number): void {
    console.log('\n\n' + '=' .repeat(70));
    console.log('\n📊 PIPELINE METRICS\n');

    const successRate = (this.metrics.successfulRequests / this.metrics.totalRequests) * 100;
    const avgDuration = this.metrics.totalDuration / this.metrics.successfulRequests;
    const cacheHitRate = (this.metrics.cacheHits / this.metrics.totalRequests) * 100;

    console.log('Performance:');
    console.log(`  Total Time: ${(totalTime / 1000).toFixed(2)}s`);
    console.log(`  Avg Request Time: ${avgDuration.toFixed(0)}ms`);
    console.log(`  Throughput: ${(this.metrics.successfulRequests / (totalTime / 1000)).toFixed(2)} req/s`);

    console.log('\nReliability:');
    console.log(`  Total Requests: ${this.metrics.totalRequests}`);
    console.log(`  Successful: ${this.metrics.successfulRequests} (${successRate.toFixed(1)}%)`);
    console.log(`  Failed: ${this.metrics.failedRequests}`);
    console.log(`  Retries: ${this.metrics.retries}`);

    console.log('\nCost & Efficiency:');
    console.log(`  Total Cost: $${this.metrics.totalCost.toFixed(4)}`);
    console.log(`  Avg Cost/Request: $${(this.metrics.totalCost / this.metrics.totalRequests).toFixed(6)}`);
    console.log(`  Cache Hit Rate: ${cacheHitRate.toFixed(1)}%`);
    console.log(`  Cost Savings from Cache: $${(this.metrics.cacheHits * 0.0001).toFixed(4)}`);

    if (this.metrics.errors.length > 0) {
      console.log(`\n⚠️  Errors (${this.metrics.errors.length}):`);
      this.metrics.errors.slice(0, 5).forEach((err, i) => {
        console.log(`  ${i + 1}. ${err.error}`);
      });
      if (this.metrics.errors.length > 5) {
        console.log(`  ... and ${this.metrics.errors.length - 5} more`);
      }
    }

    console.log('\n' + '=' .repeat(70) + '\n');
  }

  // Get metrics
  getMetrics(): PipelineMetrics {
    return { ...this.metrics };
  }
}

// Example quality validator
class ProductQualityValidator implements QualityValidator {
  validate(data: any[]): { valid: boolean; score: number; issues: string[] } {
    const issues: string[] = [];
    let score = 1.0;

    if (!Array.isArray(data) || data.length === 0) {
      return { valid: false, score: 0, issues: ['No data generated'] };
    }

    data.forEach((item, idx) => {
      if (!item.description || item.description.length < 50) {
        issues.push(`Item ${idx}: Description too short`);
        score -= 0.1;
      }

      if (!item.key_features || !Array.isArray(item.key_features) || item.key_features.length < 3) {
        issues.push(`Item ${idx}: Insufficient features`);
        score -= 0.1;
      }
    });

    score = Math.max(0, score);
    const valid = score >= 0.7;

    return { valid, score, issues };
  }
}

// Main execution
async function runProductionPipeline() {
  const pipeline = new ProductionPipeline({
    maxRetries: 3,
    retryDelay: 2000,
    batchSize: 5,
    maxConcurrency: 2,
    qualityThreshold: 0.7,
    costBudget: 1.0,
    rateLimitPerMinute: 30,
    enableCaching: true,
    outputDirectory: join(process.cwd(), 'examples', 'output', 'production')
  });

  const validator = new ProductQualityValidator();

  // Generate product data for e-commerce catalog
  const requests = [
    {
      count: 2,
      schema: {
        id: { type: 'string', required: true },
        name: { type: 'string', required: true },
        description: { type: 'string', required: true },
        key_features: { type: 'array', items: { type: 'string' }, required: true },
        price: { type: 'number', required: true, minimum: 10, maximum: 1000 },
        category: { type: 'string', enum: ['Electronics', 'Clothing', 'Home', 'Sports'] }
      }
    }
  ];

  // Duplicate requests to test batching
  const allRequests = Array(5).fill(null).map(() => requests[0]);

  const results = await pipeline.run(allRequests, validator);

  console.log(`\n✅ Pipeline complete! Generated ${results.length} batches of products.\n`);
}

// Run the example
if (import.meta.url === `file://${process.argv[1]}`) {
  runProductionPipeline().catch(error => {
    console.error('❌ Pipeline failed:', error);
    process.exit(1);
  });
}

export { ProductionPipeline, ProductQualityValidator, PipelineConfig, PipelineMetrics };
