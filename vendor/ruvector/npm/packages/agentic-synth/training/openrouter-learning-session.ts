/**
 * Comprehensive Agentic-Synth Training & Learning Session
 *
 * This script demonstrates a complete training workflow using OpenRouter API:
 * 1. Baseline generation and measurement
 * 2. Learning from successful patterns
 * 3. Adaptive optimization
 * 4. Comprehensive benchmarking
 * 5. Final optimized generation
 *
 * Usage:
 *   export OPENROUTER_API_KEY=your-key-here
 *   npx tsx training/openrouter-learning-session.ts
 */

import { AgenticSynth } from '../dist/index.js';
import type { GenerationResult } from '../src/types.js';
import { performance } from 'perf_hooks';
import * as fs from 'fs/promises';
import * as path from 'path';

// ============================================================================
// Configuration
// ============================================================================

const CONFIG = {
  provider: 'openrouter' as const,
  apiKey: process.env.OPENROUTER_API_KEY || '',
  models: [
    'anthropic/claude-3.5-sonnet', // High quality
    'openai/gpt-4-turbo',           // Balanced
    'meta-llama/llama-3.1-70b-instruct' // Fast
  ],
  outputDir: './training/results',

  // Training parameters
  generations: 5,
  samplesPerGeneration: 100,
  learningRate: 0.1,
  qualityThreshold: 0.85,

  // Benchmark parameters
  benchmarkIterations: 10,
  benchmarkSizes: [100, 500, 1000, 5000],
};

// ============================================================================
// Types
// ============================================================================

interface TrainingMetrics {
  generation: number;
  quality: number;
  diversity: number;
  speed: number;
  cacheHitRate: number;
  memoryUsage: number;
  timestamp: string;
}

interface LearningPattern {
  pattern: string;
  successRate: number;
  avgQuality: number;
  examples: any[];
}

interface BenchmarkResult {
  model: string;
  sampleSize: number;
  avgLatency: number;
  throughput: number;
  quality: number;
  cacheHitRate: number;
}

// ============================================================================
// Training Session Class
// ============================================================================

class TrainingSession {
  private synth: AgenticSynth;
  private metrics: TrainingMetrics[] = [];
  private patterns: Map<string, LearningPattern> = new Map();
  private bestSchema: any = null;
  private bestQuality: number = 0;

  constructor() {
    if (!CONFIG.apiKey) {
      throw new Error('OPENROUTER_API_KEY environment variable is required');
    }

    this.synth = new AgenticSynth({
      provider: CONFIG.provider,
      apiKey: CONFIG.apiKey,
      model: CONFIG.models[0], // Start with highest quality
      cacheStrategy: 'memory',
      cacheTTL: 3600,
      maxCacheSize: 10000,
    });
  }

  /**
   * Run complete training session
   */
  async run(): Promise<void> {
    console.log('🎓 Starting Agentic-Synth Training & Learning Session\n');
    console.log('='.repeat(70));

    // Ensure output directory exists
    await fs.mkdir(CONFIG.outputDir, { recursive: true });

    try {
      // Phase 1: Baseline Generation
      console.log('\n📊 Phase 1: Baseline Generation');
      await this.runBaselineGeneration();

      // Phase 2: Learning Loop
      console.log('\n🧠 Phase 2: Learning & Optimization Loop');
      await this.runLearningLoop();

      // Phase 3: Model Comparison
      console.log('\n🔬 Phase 3: Multi-Model Comparison');
      await this.runModelComparison();

      // Phase 4: Comprehensive Benchmarking
      console.log('\n⚡ Phase 4: Comprehensive Benchmarking');
      await this.runComprehensiveBenchmarks();

      // Phase 5: Final Optimized Generation
      console.log('\n🎯 Phase 5: Final Optimized Generation');
      await this.runOptimizedGeneration();

      // Generate Reports
      console.log('\n📈 Phase 6: Generating Reports');
      await this.generateReports();

      console.log('\n' + '='.repeat(70));
      console.log('✅ Training session completed successfully!\n');

    } catch (error: any) {
      console.error('\n❌ Training session failed:', error.message);
      throw error;
    }
  }

  /**
   * Phase 1: Baseline Generation
   */
  private async runBaselineGeneration(): Promise<void> {
    console.log('Generating baseline dataset...');

    const schema = {
      id: 'UUID',
      name: 'full name',
      email: 'valid email',
      age: 'number (18-80)',
      occupation: 'job title',
      salary: 'number (30000-200000)',
      city: 'city name',
      country: 'country name',
    };

    const start = performance.now();
    const result = await this.synth.generateStructured({
      count: CONFIG.samplesPerGeneration,
      schema,
    });
    const duration = performance.now() - start;

    // Calculate quality metrics
    const quality = this.calculateQuality(result.data);
    const diversity = this.calculateDiversity(result.data);

    // Record metrics
    this.recordMetrics({
      generation: 0,
      quality,
      diversity,
      speed: duration,
      cacheHitRate: 0,
      memoryUsage: process.memoryUsage().heapUsed / 1024 / 1024,
      timestamp: new Date().toISOString(),
    });

    console.log(`  ✅ Generated ${result.data.length} samples`);
    console.log(`  📊 Quality: ${quality.toFixed(3)}`);
    console.log(`  🎨 Diversity: ${diversity.toFixed(3)}`);
    console.log(`  ⏱️  Duration: ${duration.toFixed(0)}ms`);

    // Save baseline data
    await this.saveData('baseline', result.data);
  }

  /**
   * Phase 2: Learning Loop
   */
  private async runLearningLoop(): Promise<void> {
    let currentSchema = {
      id: 'UUID',
      name: 'full name',
      email: 'valid email',
      age: 'number (18-80)',
      occupation: 'job title',
      salary: 'number (30000-200000)',
      city: 'city name',
      country: 'country name',
    };

    for (let gen = 1; gen <= CONFIG.generations; gen++) {
      console.log(`\n  Generation ${gen}/${CONFIG.generations}`);

      const start = performance.now();
      const result = await this.synth.generateStructured({
        count: CONFIG.samplesPerGeneration,
        schema: currentSchema,
      });
      const duration = performance.now() - start;

      // Measure quality
      const quality = this.calculateQuality(result.data);
      const diversity = this.calculateDiversity(result.data);

      // Get cache stats
      const cacheStats = this.synth.cache.getStats();

      // Record metrics
      this.recordMetrics({
        generation: gen,
        quality,
        diversity,
        speed: duration,
        cacheHitRate: cacheStats.hitRate,
        memoryUsage: process.memoryUsage().heapUsed / 1024 / 1024,
        timestamp: new Date().toISOString(),
      });

      console.log(`    Quality: ${quality.toFixed(3)} (${quality > this.bestQuality ? '↑' : '↓'})`);
      console.log(`    Diversity: ${diversity.toFixed(3)}`);
      console.log(`    Cache Hit: ${(cacheStats.hitRate * 100).toFixed(1)}%`);
      console.log(`    Duration: ${duration.toFixed(0)}ms`);

      // Learn from this generation
      if (quality > CONFIG.qualityThreshold) {
        await this.learnFromSuccess(result.data, currentSchema, quality);
        console.log(`    🧠 Learned new pattern (quality: ${quality.toFixed(3)})`);
      }

      // Track best schema
      if (quality > this.bestQuality) {
        this.bestQuality = quality;
        this.bestSchema = { ...currentSchema };
        console.log(`    ⭐ New best quality: ${quality.toFixed(3)}`);
      }

      // Evolve schema based on learning
      currentSchema = await this.evolveSchema(currentSchema, quality);

      // Save generation data
      await this.saveData(`generation-${gen}`, result.data);
    }

    console.log(`\n  📚 Learned ${this.patterns.size} successful patterns`);
    console.log(`  🎯 Best quality achieved: ${this.bestQuality.toFixed(3)}`);
  }

  /**
   * Phase 3: Model Comparison
   */
  private async runModelComparison(): Promise<void> {
    const results: any[] = [];

    for (const model of CONFIG.models) {
      console.log(`\n  Testing model: ${model}`);

      // Create synth instance with this model
      const synth = new AgenticSynth({
        provider: CONFIG.provider,
        apiKey: CONFIG.apiKey,
        model,
        cacheStrategy: 'memory',
        cacheTTL: 3600,
      });

      const start = performance.now();
      const result = await synth.generateStructured({
        count: CONFIG.samplesPerGeneration,
        schema: this.bestSchema || {
          id: 'UUID',
          name: 'full name',
          email: 'valid email',
        },
      });
      const duration = performance.now() - start;

      const quality = this.calculateQuality(result.data);
      const cacheStats = synth.cache.getStats();

      results.push({
        model,
        quality,
        duration,
        cacheHitRate: cacheStats.hitRate,
        throughput: (CONFIG.samplesPerGeneration / duration) * 1000,
      });

      console.log(`    Quality: ${quality.toFixed(3)}`);
      console.log(`    Duration: ${duration.toFixed(0)}ms`);
      console.log(`    Throughput: ${((CONFIG.samplesPerGeneration / duration) * 1000).toFixed(0)} samples/s`);
    }

    // Save comparison results
    await fs.writeFile(
      path.join(CONFIG.outputDir, 'model-comparison.json'),
      JSON.stringify(results, null, 2)
    );

    // Determine best model
    const bestModel = results.reduce((best, current) =>
      current.quality > best.quality ? current : best
    );

    console.log(`\n  🏆 Best model: ${bestModel.model}`);
    console.log(`     Quality: ${bestModel.quality.toFixed(3)}`);
    console.log(`     Speed: ${bestModel.duration.toFixed(0)}ms`);
  }

  /**
   * Phase 4: Comprehensive Benchmarking
   */
  private async runComprehensiveBenchmarks(): Promise<void> {
    const benchmarks: BenchmarkResult[] = [];

    for (const size of CONFIG.benchmarkSizes) {
      console.log(`\n  Benchmarking ${size} samples...`);

      const times: number[] = [];
      const qualities: number[] = [];

      for (let i = 0; i < CONFIG.benchmarkIterations; i++) {
        const start = performance.now();
        const result = await this.synth.generateStructured({
          count: size,
          schema: this.bestSchema,
        });
        const duration = performance.now() - start;

        times.push(duration);
        qualities.push(this.calculateQuality(result.data));

        process.stdout.write(`    Iteration ${i + 1}/${CONFIG.benchmarkIterations}\r`);
      }

      const avgLatency = times.reduce((a, b) => a + b) / times.length;
      const avgQuality = qualities.reduce((a, b) => a + b) / qualities.length;
      const throughput = (size / avgLatency) * 1000;

      const cacheStats = this.synth.cache.getStats();

      benchmarks.push({
        model: CONFIG.models[0],
        sampleSize: size,
        avgLatency,
        throughput,
        quality: avgQuality,
        cacheHitRate: cacheStats.hitRate,
      });

      console.log(`    Avg Latency: ${avgLatency.toFixed(0)}ms`);
      console.log(`    Throughput: ${throughput.toFixed(0)} samples/s`);
      console.log(`    Quality: ${avgQuality.toFixed(3)}`);
      console.log(`    Cache Hit: ${(cacheStats.hitRate * 100).toFixed(1)}%`);
    }

    // Save benchmark results
    await fs.writeFile(
      path.join(CONFIG.outputDir, 'benchmarks.json'),
      JSON.stringify(benchmarks, null, 2)
    );
  }

  /**
   * Phase 5: Final Optimized Generation
   */
  private async runOptimizedGeneration(): Promise<void> {
    console.log('Generating final optimized dataset...');

    const start = performance.now();
    const result = await this.synth.generateStructured({
      count: CONFIG.samplesPerGeneration * 10, // 10x larger
      schema: this.bestSchema,
    });
    const duration = performance.now() - start;

    const quality = this.calculateQuality(result.data);
    const diversity = this.calculateDiversity(result.data);
    const cacheStats = this.synth.cache.getStats();

    console.log(`  ✅ Generated ${result.data.length} samples`);
    console.log(`  📊 Quality: ${quality.toFixed(3)}`);
    console.log(`  🎨 Diversity: ${diversity.toFixed(3)}`);
    console.log(`  ⚡ Throughput: ${((result.data.length / duration) * 1000).toFixed(0)} samples/s`);
    console.log(`  💾 Cache Hit: ${(cacheStats.hitRate * 100).toFixed(1)}%`);
    console.log(`  ⏱️  Duration: ${(duration / 1000).toFixed(2)}s`);

    // Save optimized data
    await this.saveData('optimized-final', result.data);

    // Calculate improvement
    const baselineQuality = this.metrics[0].quality;
    const improvement = ((quality - baselineQuality) / baselineQuality) * 100;

    console.log(`\n  📈 Improvement over baseline: ${improvement >= 0 ? '+' : ''}${improvement.toFixed(1)}%`);
  }

  /**
   * Phase 6: Generate Reports
   */
  private async generateReports(): Promise<void> {
    // Save metrics history
    await fs.writeFile(
      path.join(CONFIG.outputDir, 'metrics-history.json'),
      JSON.stringify(this.metrics, null, 2)
    );

    // Save learned patterns
    const patternsArray = Array.from(this.patterns.values());
    await fs.writeFile(
      path.join(CONFIG.outputDir, 'learned-patterns.json'),
      JSON.stringify(patternsArray, null, 2)
    );

    // Generate markdown report
    const report = this.generateMarkdownReport();
    await fs.writeFile(
      path.join(CONFIG.outputDir, 'TRAINING_REPORT.md'),
      report
    );

    console.log(`  ✅ Reports saved to ${CONFIG.outputDir}/`);
    console.log(`     - metrics-history.json`);
    console.log(`     - learned-patterns.json`);
    console.log(`     - benchmarks.json`);
    console.log(`     - model-comparison.json`);
    console.log(`     - TRAINING_REPORT.md`);
  }

  // ============================================================================
  // Helper Methods
  // ============================================================================

  /**
   * Calculate quality score for generated data
   */
  private calculateQuality(data: any[]): number {
    if (data.length === 0) return 0;

    let score = 0;
    let checks = 0;

    for (const item of data.slice(0, 10)) { // Sample first 10
      // Check completeness
      const fields = Object.keys(item);
      score += fields.length > 0 ? 1 : 0;
      checks++;

      // Check data types
      if (typeof item.id === 'string') score += 1;
      if (typeof item.name === 'string' && item.name.length > 3) score += 1;
      if (typeof item.email === 'string' && item.email.includes('@')) score += 1;
      if (typeof item.age === 'number' && item.age >= 18 && item.age <= 80) score += 1;
      checks += 4;

      // Check uniqueness
      if (item.id && item.id.length > 10) score += 1;
      checks++;
    }

    return score / checks;
  }

  /**
   * Calculate diversity score
   */
  private calculateDiversity(data: any[]): number {
    if (data.length < 2) return 0;

    const uniqueValues = new Set();
    let totalFields = 0;

    for (const item of data.slice(0, 20)) {
      for (const value of Object.values(item)) {
        uniqueValues.add(JSON.stringify(value));
        totalFields++;
      }
    }

    return uniqueValues.size / totalFields;
  }

  /**
   * Record training metrics
   */
  private recordMetrics(metrics: TrainingMetrics): void {
    this.metrics.push(metrics);
  }

  /**
   * Learn from successful generation
   */
  private async learnFromSuccess(
    data: any[],
    schema: any,
    quality: number
  ): Promise<void> {
    const patternKey = JSON.stringify(schema);

    if (this.patterns.has(patternKey)) {
      const pattern = this.patterns.get(patternKey)!;
      pattern.successRate += 1;
      pattern.avgQuality = (pattern.avgQuality + quality) / 2;
      pattern.examples.push(...data.slice(0, 3));
    } else {
      this.patterns.set(patternKey, {
        pattern: patternKey,
        successRate: 1,
        avgQuality: quality,
        examples: data.slice(0, 3),
      });
    }
  }

  /**
   * Evolve schema based on learning
   */
  private async evolveSchema(currentSchema: any, quality: number): Promise<any> {
    // If quality is high, keep schema
    if (quality >= CONFIG.qualityThreshold) {
      return currentSchema;
    }

    // Otherwise, try adding a field
    const newSchema = { ...currentSchema };

    // Randomly add a new field
    const possibleFields = [
      { phone: 'phone number' },
      { address: 'street address' },
      { company: 'company name' },
      { skills: 'array of 3-5 skills' },
      { bio: 'short bio (1-2 sentences)' },
    ];

    const randomField = possibleFields[Math.floor(Math.random() * possibleFields.length)];
    Object.assign(newSchema, randomField);

    return newSchema;
  }

  /**
   * Save data to file
   */
  private async saveData(name: string, data: any[]): Promise<void> {
    const filepath = path.join(CONFIG.outputDir, `${name}.json`);
    await fs.writeFile(filepath, JSON.stringify(data, null, 2));
  }

  /**
   * Generate markdown report
   */
  private generateMarkdownReport(): string {
    const baseline = this.metrics[0];
    const final = this.metrics[this.metrics.length - 1];
    const improvement = ((final.quality - baseline.quality) / baseline.quality) * 100;

    return `# Agentic-Synth Training Report

**Date**: ${new Date().toISOString()}
**Provider**: ${CONFIG.provider}
**Model**: ${CONFIG.models[0]}

## Summary

- **Generations**: ${CONFIG.generations}
- **Samples per Generation**: ${CONFIG.samplesPerGeneration}
- **Total Samples Generated**: ${CONFIG.samplesPerGeneration * (CONFIG.generations + 1)}
- **Patterns Learned**: ${this.patterns.size}

## Quality Improvement

| Metric | Baseline | Final | Change |
|--------|----------|-------|--------|
| Quality | ${baseline.quality.toFixed(3)} | ${final.quality.toFixed(3)} | ${improvement >= 0 ? '+' : ''}${improvement.toFixed(1)}% |
| Diversity | ${baseline.diversity.toFixed(3)} | ${final.diversity.toFixed(3)} | ${(((final.diversity - baseline.diversity) / baseline.diversity) * 100).toFixed(1)}% |
| Speed | ${baseline.speed.toFixed(0)}ms | ${final.speed.toFixed(0)}ms | ${(((final.speed - baseline.speed) / baseline.speed) * 100).toFixed(1)}% |
| Cache Hit | ${(baseline.cacheHitRate * 100).toFixed(1)}% | ${(final.cacheHitRate * 100).toFixed(1)}% | +${((final.cacheHitRate - baseline.cacheHitRate) * 100).toFixed(1)}% |

## Training Progress

${this.metrics.map((m, i) => `
### Generation ${i}

- Quality: ${m.quality.toFixed(3)}
- Diversity: ${m.diversity.toFixed(3)}
- Speed: ${m.speed.toFixed(0)}ms
- Cache Hit: ${(m.cacheHitRate * 100).toFixed(1)}%
- Memory: ${m.memoryUsage.toFixed(0)}MB
`).join('\n')}

## Learned Patterns

Total patterns learned: ${this.patterns.size}

${Array.from(this.patterns.values()).map(p => `
- Success Rate: ${p.successRate}
- Avg Quality: ${p.avgQuality.toFixed(3)}
`).join('\n')}

## Best Configuration

\`\`\`json
${JSON.stringify(this.bestSchema, null, 2)}
\`\`\`

**Best Quality Achieved**: ${this.bestQuality.toFixed(3)}

## Recommendations

${improvement > 10 ? '✅' : '⚠️'} Quality improvement: ${improvement.toFixed(1)}%
${final.cacheHitRate > 0.7 ? '✅' : '⚠️'} Cache hit rate: ${(final.cacheHitRate * 100).toFixed(1)}%
${this.patterns.size >= 3 ? '✅' : '⚠️'} Patterns learned: ${this.patterns.size}

## Next Steps

1. ${improvement < 10 ? 'Increase learning rate or generation count' : 'Continue with current parameters'}
2. ${final.cacheHitRate < 0.7 ? 'Optimize caching strategy' : 'Cache performance is good'}
3. ${this.patterns.size < 3 ? 'Generate more diverse schemas' : 'Explore schema variations'}

---

Generated by agentic-synth v0.1.0
`;
  }
}

// ============================================================================
// Main Execution
// ============================================================================

async function main() {
  try {
    const session = new TrainingSession();
    await session.run();
  } catch (error: any) {
    console.error('Fatal error:', error.message);
    process.exit(1);
  }
}

// Run if executed directly
if (import.meta.url === `file://${process.argv[1]}`) {
  main();
}

export { TrainingSession };
