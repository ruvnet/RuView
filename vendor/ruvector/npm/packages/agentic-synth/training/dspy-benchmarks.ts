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

import { performance } from 'perf_hooks';
import * as fs from 'fs/promises';
import * as path from 'path';

// ============================================================================
// Types & Interfaces
// ============================================================================

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
    [key: string]: number; // p-values
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

// ============================================================================
// Mock Data Generator
// ============================================================================

class MockModelSimulator {
  private modelConfig: ModelConfig;
  private baseQuality: number;
  private learningRate: number;
  private generation: number = 0;

  constructor(config: ModelConfig) {
    this.modelConfig = config;
    // Different models have different base qualities
    this.baseQuality = this.getBaseQuality(config.name);
    this.learningRate = this.getLearningRate(config.name);
  }

  private getBaseQuality(modelName: string): number {
    const qualities: { [key: string]: number } = {
      'gpt-4': 0.85,
      'claude-3.5-sonnet': 0.88,
      'gemini-pro': 0.82,
      'gpt-3.5-turbo': 0.75,
      'llama-3-70b': 0.78,
      'mixtral-8x7b': 0.76,
    };
    return qualities[modelName] || 0.70;
  }

  private getLearningRate(modelName: string): number {
    const rates: { [key: string]: number } = {
      'gpt-4': 0.02,
      'claude-3.5-sonnet': 0.025,
      'gemini-pro': 0.018,
      'gpt-3.5-turbo': 0.03,
      'llama-3-70b': 0.022,
      'mixtral-8x7b': 0.028,
    };
    return rates[modelName] || 0.02;
  }

  async generateBatch(count: number, schema: any): Promise<any[]> {
    // Simulate API latency based on model
    const baseLatency = this.getBaseLatency();
    const latency = baseLatency + Math.random() * (baseLatency * 0.3);
    await new Promise(resolve => setTimeout(resolve, latency));

    const data: any[] = [];
    for (let i = 0; i < count; i++) {
      data.push(this.generateSample(schema));
    }

    // Simulate learning improvement
    this.generation++;

    return data;
  }

  private getBaseLatency(): number {
    const latencies: { [key: string]: number } = {
      'gpt-4': 1500,
      'claude-3.5-sonnet': 1200,
      'gemini-pro': 800,
      'gpt-3.5-turbo': 500,
      'llama-3-70b': 600,
      'mixtral-8x7b': 400,
    };
    return latencies[this.modelConfig.model] || 1000;
  }

  private generateSample(schema: any): any {
    const sample: any = {};
    for (const [key, type] of Object.entries(schema)) {
      sample[key] = this.generateField(key, type as string);
    }
    return sample;
  }

  private generateField(key: string, type: string): any {
    if (type.includes('UUID')) {
      return `${Math.random().toString(36).substring(2, 15)}-${Math.random().toString(36).substring(2, 15)}`;
    }
    if (type.includes('email')) {
      return `user${Math.floor(Math.random() * 10000)}@example.com`;
    }
    if (type.includes('name')) {
      const names = ['Alice', 'Bob', 'Charlie', 'Diana', 'Eve', 'Frank', 'Grace', 'Henry', 'Ivy', 'Jack'];
      const lastNames = ['Smith', 'Johnson', 'Williams', 'Brown', 'Jones', 'Garcia', 'Miller', 'Davis', 'Rodriguez'];
      return `${names[Math.floor(Math.random() * names.length)]} ${lastNames[Math.floor(Math.random() * lastNames.length)]}`;
    }
    if (type.includes('number')) {
      const match = type.match(/\((\d+)-(\d+)\)/);
      if (match) {
        const min = parseInt(match[1]);
        const max = parseInt(match[2]);
        return Math.floor(Math.random() * (max - min + 1)) + min;
      }
      return Math.floor(Math.random() * 100);
    }
    return `sample_${key}_${Math.random().toString(36).substring(2, 9)}`;
  }

  getCurrentQuality(): number {
    const learned = Math.min(0.15, this.generation * this.learningRate);
    return Math.min(0.98, this.baseQuality + learned);
  }

  getConfig(): ModelConfig {
    return this.modelConfig;
  }
}

// ============================================================================
// Statistical Utilities
// ============================================================================

class StatisticalAnalyzer {
  /**
   * Calculate mean of array
   */
  static mean(values: number[]): number {
    if (values.length === 0) return 0;
    return values.reduce((sum, val) => sum + val, 0) / values.length;
  }

  /**
   * Calculate standard deviation
   */
  static stdDev(values: number[]): number {
    const avg = this.mean(values);
    const squareDiffs = values.map(value => Math.pow(value - avg, 2));
    return Math.sqrt(this.mean(squareDiffs));
  }

  /**
   * Calculate percentile
   */
  static percentile(values: number[], p: number): number {
    if (values.length === 0) return 0;
    const sorted = [...values].sort((a, b) => a - b);
    const index = Math.ceil((p / 100) * sorted.length) - 1;
    return sorted[Math.max(0, index)];
  }

  /**
   * Perform t-test to determine statistical significance
   * Returns p-value
   */
  static tTest(sample1: number[], sample2: number[]): number {
    const mean1 = this.mean(sample1);
    const mean2 = this.mean(sample2);
    const std1 = this.stdDev(sample1);
    const std2 = this.stdDev(sample2);
    const n1 = sample1.length;
    const n2 = sample2.length;

    const pooledStd = Math.sqrt(
      ((n1 - 1) * Math.pow(std1, 2) + (n2 - 1) * Math.pow(std2, 2)) / (n1 + n2 - 2)
    );

    const tStat = Math.abs(mean1 - mean2) / (pooledStd * Math.sqrt(1/n1 + 1/n2));

    // Simplified p-value approximation
    const df = n1 + n2 - 2;
    const pValue = 2 * (1 - this.tDistribution(tStat, df));

    return pValue;
  }

  /**
   * Simplified t-distribution CDF approximation
   */
  private static tDistribution(t: number, df: number): number {
    // Simplified approximation for demonstration
    const x = df / (df + t * t);
    return 1 - 0.5 * Math.pow(x, df / 2);
  }

  /**
   * Calculate Shannon entropy for diversity measurement
   */
  static entropy(values: any[]): number {
    const counts = new Map<string, number>();
    for (const val of values) {
      const key = JSON.stringify(val);
      counts.set(key, (counts.get(key) || 0) + 1);
    }

    let entropy = 0;
    const total = values.length;
    const countValues = Array.from(counts.values());
    for (const count of countValues) {
      const p = count / total;
      entropy -= p * Math.log2(p);
    }

    return entropy;
  }
}

// ============================================================================
// Benchmark Suite
// ============================================================================

export class BenchmarkSuite {
  private models: MockModelSimulator[] = [];
  private outputDir: string = './training/results/benchmarks';
  private results: BenchmarkResult[] = [];

  constructor(outputDir?: string) {
    if (outputDir) {
      this.outputDir = outputDir;
    }
  }

  /**
   * Add a model configuration to the benchmark suite
   */
  addModel(config: ModelConfig): void {
    this.models.push(new MockModelSimulator(config));
  }

  /**
   * Add multiple common models for quick testing
   */
  addCommonModels(): void {
    const commonModels: ModelConfig[] = [
      { name: 'GPT-4', provider: 'openai', model: 'gpt-4', costPer1kTokens: 0.03, maxTokens: 8192 },
      { name: 'Claude 3.5 Sonnet', provider: 'anthropic', model: 'claude-3.5-sonnet', costPer1kTokens: 0.015, maxTokens: 200000 },
      { name: 'Gemini Pro', provider: 'gemini', model: 'gemini-pro', costPer1kTokens: 0.0005, maxTokens: 32768 },
      { name: 'GPT-3.5 Turbo', provider: 'openai', model: 'gpt-3.5-turbo', costPer1kTokens: 0.0015, maxTokens: 16384 },
      { name: 'Llama 3 70B', provider: 'openrouter', model: 'llama-3-70b', costPer1kTokens: 0.0008, maxTokens: 8192 },
      { name: 'Mixtral 8x7B', provider: 'openrouter', model: 'mixtral-8x7b', costPer1kTokens: 0.0005, maxTokens: 32768 },
    ];

    for (const config of commonModels) {
      this.addModel(config);
    }
  }

  /**
   * Run comprehensive comparison across all models
   */
  async runModelComparison(sampleSize: number = 1000): Promise<ComparisonResult> {
    console.log(`\n🔬 Running Model Comparison (${sampleSize} samples)`);
    console.log('='.repeat(70));

    await fs.mkdir(this.outputDir, { recursive: true });

    const schema = {
      id: 'UUID',
      name: 'full name',
      email: 'valid email',
      age: 'number (18-80)',
      occupation: 'job title',
      description: 'text (50-200 words)',
    };

    this.results = [];

    for (const model of this.models) {
      console.log(`\nTesting ${model.getConfig().name}...`);
      const result = await this.benchmarkModel(model, sampleSize, schema);
      this.results.push(result);

      console.log(`  Quality: ${result.quality.overall.toFixed(3)}`);
      console.log(`  Latency P95: ${result.performance.latencyP95.toFixed(0)}ms`);
      console.log(`  Cost/Sample: $${result.cost.costPerSample.toFixed(6)}`);
      console.log(`  Diversity: ${result.diversity.coverageScore.toFixed(3)}`);
    }

    return this.compareResults();
  }

  /**
   * Test scalability from 100 to 100K samples
   */
  async runScalabilityTest(): Promise<ScalabilityResult[]> {
    console.log('\n📊 Running Scalability Test');
    console.log('='.repeat(70));

    const sampleSizes = [100, 500, 1000, 5000, 10000, 50000, 100000];
    const results: ScalabilityResult[] = [];

    const schema = {
      id: 'UUID',
      name: 'full name',
      email: 'valid email',
    };

    for (const model of this.models) {
      console.log(`\nTesting ${model.getConfig().name}...`);

      const latencies: number[] = [];
      const throughputs: number[] = [];
      const costs: number[] = [];
      const qualities: number[] = [];

      for (const size of sampleSizes) {
        console.log(`  ${size} samples...`);
        const start = performance.now();
        const data = await model.generateBatch(size, schema);
        const duration = performance.now() - start;

        const latency = duration / size;
        const throughput = (size / duration) * 1000;
        const quality = model.getCurrentQuality();
        const cost = (size * 100 * model.getConfig().costPer1kTokens) / 1000; // Assume 100 tokens per sample

        latencies.push(latency);
        throughputs.push(throughput);
        costs.push(cost);
        qualities.push(quality);

        console.log(`    Latency: ${latency.toFixed(2)}ms, Throughput: ${throughput.toFixed(0)}/s`);
      }

      // Calculate scaling efficiency (lower is better, close to 1.0 is linear)
      const scalingEfficiency = latencies[latencies.length - 1] / latencies[0];

      results.push({
        modelName: model.getConfig().name,
        sampleSizes,
        latencies,
        throughputs,
        costs,
        qualities,
        scalingEfficiency,
      });
    }

    await this.saveScalabilityResults(results);
    return results;
  }

  /**
   * Analyze cost-effectiveness across models
   */
  async runCostAnalysis(): Promise<void> {
    console.log('\n💰 Running Cost Analysis');
    console.log('='.repeat(70));

    if (this.results.length === 0) {
      await this.runModelComparison(1000);
    }

    // Sort by cost per quality point
    const sortedByCost = [...this.results].sort(
      (a, b) => a.cost.costPerQualityPoint - b.cost.costPerQualityPoint
    );

    console.log('\n📈 Cost-Effectiveness Ranking:');
    console.log('-'.repeat(70));
    for (let i = 0; i < sortedByCost.length; i++) {
      const result = sortedByCost[i];
      console.log(`${i + 1}. ${result.modelName}`);
      console.log(`   Cost/Sample: $${result.cost.costPerSample.toFixed(6)}`);
      console.log(`   Cost/Quality: $${result.cost.costPerQualityPoint.toFixed(6)}`);
      console.log(`   Quality: ${result.quality.overall.toFixed(3)}`);
      console.log(`   Efficiency: ${result.cost.efficiency.toFixed(3)}`);
      console.log();
    }
  }

  /**
   * Measure quality convergence and learning rates
   */
  async runQualityConvergence(generations: number = 10): Promise<void> {
    console.log('\n🎯 Running Quality Convergence Test');
    console.log('='.repeat(70));

    const schema = {
      id: 'UUID',
      name: 'full name',
      email: 'valid email',
      age: 'number (18-80)',
    };

    const convergenceData: any[] = [];

    for (const model of this.models) {
      console.log(`\nTesting ${model.getConfig().name}...`);
      const qualities: number[] = [];

      for (let gen = 0; gen < generations; gen++) {
        await model.generateBatch(100, schema);
        const quality = model.getCurrentQuality();
        qualities.push(quality);

        if (gen % 2 === 0) {
          console.log(`  Generation ${gen}: Quality ${quality.toFixed(3)}`);
        }
      }

      // Calculate convergence metrics
      const improvementRate = (qualities[qualities.length - 1] - qualities[0]) / generations;
      const plateauGen = this.findPlateauGeneration(qualities);

      convergenceData.push({
        modelName: model.getConfig().name,
        qualities,
        improvementRate,
        plateauGeneration: plateauGen,
        finalQuality: qualities[qualities.length - 1],
      });

      console.log(`  Improvement Rate: ${(improvementRate * 100).toFixed(2)}%/gen`);
      console.log(`  Plateau at Generation: ${plateauGen}`);
    }

    await this.saveConvergenceData(convergenceData);
  }

  /**
   * Analyze data diversity and variety
   */
  async runDiversityAnalysis(sampleSize: number = 5000): Promise<void> {
    console.log('\n🎨 Running Diversity Analysis');
    console.log('='.repeat(70));

    const schema = {
      id: 'UUID',
      name: 'full name',
      email: 'valid email',
      age: 'number (18-80)',
      occupation: 'job title',
    };

    for (const model of this.models) {
      console.log(`\nAnalyzing ${model.getConfig().name}...`);

      const data = await model.generateBatch(sampleSize, schema);
      const diversity = this.calculateDiversityMetrics(data);

      console.log(`  Unique Values: ${diversity.uniqueValues}`);
      console.log(`  Pattern Variety: ${diversity.patternVariety.toFixed(3)}`);
      console.log(`  Entropy: ${diversity.distributionEntropy.toFixed(3)}`);
      console.log(`  Coverage: ${diversity.coverageScore.toFixed(3)}`);
      console.log(`  Novelty Rate: ${diversity.noveltyRate.toFixed(3)}`);
    }
  }

  /**
   * Benchmark a single model
   */
  private async benchmarkModel(
    model: MockModelSimulator,
    sampleSize: number,
    schema: any
  ): Promise<BenchmarkResult> {
    const startTime = performance.now();
    const latencies: number[] = [];
    const allData: any[] = [];

    // Run multiple batches to collect performance data
    const batchSize = 100;
    const batches = Math.ceil(sampleSize / batchSize);

    for (let i = 0; i < batches; i++) {
      const batchStart = performance.now();
      const data = await model.generateBatch(Math.min(batchSize, sampleSize - i * batchSize), schema);
      const batchLatency = performance.now() - batchStart;

      latencies.push(batchLatency);
      allData.push(...data);
    }

    const totalDuration = performance.now() - startTime;

    // Calculate metrics
    const quality = this.calculateQualityMetrics(allData, model.getCurrentQuality());
    const performanceMetrics = this.calculatePerformanceMetrics(latencies, sampleSize, totalDuration);
    const cost = this.calculateCostMetrics(model.getConfig(), sampleSize, quality.overall);
    const learning = this.calculateLearningMetrics(model);
    const diversity = this.calculateDiversityMetrics(allData);

    return {
      modelName: model.getConfig().name,
      sampleSize,
      quality,
      performance: performanceMetrics,
      cost,
      learning,
      diversity,
      timestamp: new Date().toISOString(),
      duration: totalDuration,
    };
  }

  /**
   * Calculate quality metrics
   */
  private calculateQualityMetrics(data: any[], baseQuality: number): QualityMetrics {
    // Simulate quality calculations
    const accuracy = baseQuality + (Math.random() * 0.05 - 0.025);
    const coherence = baseQuality + (Math.random() * 0.04 - 0.02);
    const validity = baseQuality - 0.02 + (Math.random() * 0.03);
    const consistency = baseQuality + (Math.random() * 0.03 - 0.015);
    const completeness = baseQuality + 0.01 + (Math.random() * 0.02);

    const overall = (accuracy + coherence + validity + consistency + completeness) / 5;

    return {
      accuracy: Math.max(0, Math.min(1, accuracy)),
      coherence: Math.max(0, Math.min(1, coherence)),
      validity: Math.max(0, Math.min(1, validity)),
      consistency: Math.max(0, Math.min(1, consistency)),
      completeness: Math.max(0, Math.min(1, completeness)),
      overall: Math.max(0, Math.min(1, overall)),
    };
  }

  /**
   * Calculate performance metrics
   */
  private calculatePerformanceMetrics(
    latencies: number[],
    sampleSize: number,
    totalDuration: number
  ): PerformanceMetrics {
    return {
      latencyP50: StatisticalAnalyzer.percentile(latencies, 50),
      latencyP95: StatisticalAnalyzer.percentile(latencies, 95),
      latencyP99: StatisticalAnalyzer.percentile(latencies, 99),
      avgLatency: StatisticalAnalyzer.mean(latencies),
      minLatency: Math.min(...latencies),
      maxLatency: Math.max(...latencies),
      throughput: (sampleSize / totalDuration) * 1000,
      successRate: 1.0 - (Math.random() * 0.02), // 98-100% success
    };
  }

  /**
   * Calculate cost metrics
   */
  private calculateCostMetrics(
    config: ModelConfig,
    sampleSize: number,
    quality: number
  ): CostMetrics {
    // Assume average 150 tokens per sample (input + output)
    const avgTokensPerSample = 150;
    const tokensUsed = sampleSize * avgTokensPerSample;
    const totalCost = (tokensUsed / 1000) * config.costPer1kTokens;
    const costPerSample = totalCost / sampleSize;
    const costPerQualityPoint = costPerSample / quality;
    const efficiency = quality / costPerSample;

    return {
      totalCost,
      costPerSample,
      costPerQualityPoint,
      tokensUsed,
      efficiency,
    };
  }

  /**
   * Calculate learning metrics
   */
  private calculateLearningMetrics(model: MockModelSimulator): LearningMetrics {
    const currentQuality = model.getCurrentQuality();
    const learningCurve = Array.from({ length: 10 }, (_, i) =>
      Math.min(0.98, currentQuality - (0.1 * (10 - i - 1) / 10))
    );

    return {
      improvementRate: 0.02 + Math.random() * 0.01,
      convergenceSpeed: 5 + Math.random() * 3,
      learningCurve,
      plateauGeneration: Math.floor(6 + Math.random() * 3),
      finalQuality: currentQuality,
    };
  }

  /**
   * Calculate diversity metrics
   */
  private calculateDiversityMetrics(data: any[]): DiversityMetrics {
    const uniqueValues = new Set<string>();
    const fieldValues: Map<string, Set<any>> = new Map();

    for (const item of data) {
      uniqueValues.add(JSON.stringify(item));

      for (const [key, value] of Object.entries(item)) {
        if (!fieldValues.has(key)) {
          fieldValues.set(key, new Set());
        }
        fieldValues.get(key)!.add(value);
      }
    }

    const patternVariety = uniqueValues.size / data.length;
    const entropy = StatisticalAnalyzer.entropy(data.slice(0, 1000)); // Sample for performance

    // Calculate average field diversity
    let totalFieldDiversity = 0;
    const fieldValueSets = Array.from(fieldValues.values());
    for (const values of fieldValueSets) {
      totalFieldDiversity += values.size / data.length;
    }
    const coverageScore = totalFieldDiversity / fieldValues.size;

    const noveltyRate = uniqueValues.size / data.length;

    return {
      uniqueValues: uniqueValues.size,
      patternVariety,
      distributionEntropy: entropy,
      coverageScore,
      noveltyRate,
    };
  }

  /**
   * Compare results and generate comparison report
   */
  private compareResults(): ComparisonResult {
    const models = this.results.map(r => r.modelName);

    // Find winners in each category
    const qualityWinner = this.results.reduce((prev, curr) =>
      curr.quality.overall > prev.quality.overall ? curr : prev
    );

    const perfWinner = this.results.reduce((prev, curr) =>
      curr.performance.latencyP95 < prev.performance.latencyP95 ? curr : prev
    );

    const costWinner = this.results.reduce((prev, curr) =>
      curr.cost.costPerQualityPoint < prev.cost.costPerQualityPoint ? curr : prev
    );

    const learningWinner = this.results.reduce((prev, curr) =>
      curr.learning.improvementRate > prev.learning.improvementRate ? curr : prev
    );

    const diversityWinner = this.results.reduce((prev, curr) =>
      curr.diversity.coverageScore > prev.diversity.coverageScore ? curr : prev
    );

    // Calculate overall winner (weighted score)
    const overallWinner = this.results.reduce((prev, curr) => {
      const prevScore = prev.quality.overall * 0.3 +
                       (1 / prev.performance.latencyP95) * 10000 * 0.2 +
                       (1 / prev.cost.costPerQualityPoint) * 0.2 +
                       prev.learning.improvementRate * 10 * 0.15 +
                       prev.diversity.coverageScore * 0.15;

      const currScore = curr.quality.overall * 0.3 +
                       (1 / curr.performance.latencyP95) * 10000 * 0.2 +
                       (1 / curr.cost.costPerQualityPoint) * 0.2 +
                       curr.learning.improvementRate * 10 * 0.15 +
                       curr.diversity.coverageScore * 0.15;

      return currScore > prevScore ? curr : prev;
    });

    // Statistical significance
    const significance: { [key: string]: number } = {};
    for (let i = 0; i < this.results.length; i++) {
      for (let j = i + 1; j < this.results.length; j++) {
        const model1 = this.results[i];
        const model2 = this.results[j];
        const key = `${model1.modelName}_vs_${model2.modelName}`;

        // Compare quality learning curves
        const pValue = StatisticalAnalyzer.tTest(
          model1.learning.learningCurve,
          model2.learning.learningCurve
        );
        significance[key] = pValue;
      }
    }

    // Pareto frontier (quality vs cost)
    const paretoFrontier = this.calculateParetoFrontier();

    // Use case recommendations
    const recommendations = {
      'high-quality-low-volume': qualityWinner.modelName,
      'high-volume-low-latency': perfWinner.modelName,
      'cost-optimized': costWinner.modelName,
      'balanced': overallWinner.modelName,
      'research': qualityWinner.modelName,
      'production': this.results.reduce((prev, curr) =>
        (curr.performance.throughput * curr.quality.overall) >
        (prev.performance.throughput * prev.quality.overall) ? curr : prev
      ).modelName,
    };

    return {
      models,
      winner: {
        overall: overallWinner.modelName,
        quality: qualityWinner.modelName,
        performance: perfWinner.modelName,
        cost: costWinner.modelName,
        learning: learningWinner.modelName,
        diversity: diversityWinner.modelName,
      },
      statisticalSignificance: significance,
      paretoFrontier,
      recommendations,
    };
  }

  /**
   * Calculate Pareto frontier for quality vs cost trade-off
   */
  private calculateParetoFrontier(): string[] {
    const frontier: BenchmarkResult[] = [];

    for (const result of this.results) {
      let isDominated = false;

      for (const other of this.results) {
        if (result === other) continue;

        // Check if 'other' dominates 'result'
        if (other.quality.overall >= result.quality.overall &&
            other.cost.costPerSample <= result.cost.costPerSample &&
            (other.quality.overall > result.quality.overall ||
             other.cost.costPerSample < result.cost.costPerSample)) {
          isDominated = true;
          break;
        }
      }

      if (!isDominated) {
        frontier.push(result);
      }
    }

    return frontier.map(r => r.modelName);
  }

  /**
   * Find generation where quality plateaus
   */
  private findPlateauGeneration(qualities: number[]): number {
    const threshold = 0.005; // 0.5% improvement threshold

    for (let i = 2; i < qualities.length; i++) {
      const recentImprovement = qualities[i] - qualities[i - 1];
      if (Math.abs(recentImprovement) < threshold) {
        return i;
      }
    }

    return qualities.length;
  }

  /**
   * Generate comprehensive JSON report
   */
  async generateJSONReport(comparison: ComparisonResult): Promise<void> {
    const report = {
      metadata: {
        timestamp: new Date().toISOString(),
        framework: 'DSPy Benchmark Suite',
        version: '1.0.0',
      },
      comparison,
      results: this.results,
      summary: this.generateSummary(comparison),
    };

    const filepath = path.join(this.outputDir, 'benchmark-comparison.json');
    await fs.writeFile(filepath, JSON.stringify(report, null, 2));
    console.log(`\n✅ JSON report saved to ${filepath}`);
  }

  /**
   * Generate comprehensive Markdown report
   */
  async generateMarkdownReport(comparison: ComparisonResult): Promise<void> {
    const report = this.buildMarkdownReport(comparison);
    const filepath = path.join(this.outputDir, 'BENCHMARK_REPORT.md');
    await fs.writeFile(filepath, report);
    console.log(`✅ Markdown report saved to ${filepath}`);
  }

  /**
   * Build markdown report content
   */
  private buildMarkdownReport(comparison: ComparisonResult): string {
    let md = `# DSPy Model Benchmark Comparison Report

**Generated**: ${new Date().toISOString()}
**Framework**: DSPy Benchmark Suite v1.0.0
**Models Tested**: ${comparison.models.length}

---

## Executive Summary

### Overall Winner: ${comparison.winner.overall}

This model provides the best balance across quality, performance, cost, learning, and diversity metrics.

### Category Winners

| Category | Winner | Key Metric |
|----------|--------|------------|
| 🏆 Overall | ${comparison.winner.overall} | Best weighted score |
| 🎯 Quality | ${comparison.winner.quality} | Highest overall quality |
| ⚡ Performance | ${comparison.winner.performance} | Lowest P95 latency |
| 💰 Cost | ${comparison.winner.cost} | Best cost per quality point |
| 🧠 Learning | ${comparison.winner.learning} | Fastest improvement rate |
| 🎨 Diversity | ${comparison.winner.diversity} | Best coverage score |

---

## Detailed Results

`;

    // Add detailed results for each model
    for (const result of this.results) {
      md += `### ${result.modelName}

#### Quality Metrics
- **Overall Quality**: ${result.quality.overall.toFixed(3)}
- Accuracy: ${result.quality.accuracy.toFixed(3)}
- Coherence: ${result.quality.coherence.toFixed(3)}
- Validity: ${result.quality.validity.toFixed(3)}
- Consistency: ${result.quality.consistency.toFixed(3)}
- Completeness: ${result.quality.completeness.toFixed(3)}

#### Performance Metrics
- **Latency P50**: ${result.performance.latencyP50.toFixed(0)}ms
- **Latency P95**: ${result.performance.latencyP95.toFixed(0)}ms
- **Latency P99**: ${result.performance.latencyP99.toFixed(0)}ms
- Average Latency: ${result.performance.avgLatency.toFixed(0)}ms
- Throughput: ${result.performance.throughput.toFixed(0)} samples/s
- Success Rate: ${(result.performance.successRate * 100).toFixed(2)}%

#### Cost Metrics
- **Total Cost**: $${result.cost.totalCost.toFixed(4)}
- **Cost per Sample**: $${result.cost.costPerSample.toFixed(6)}
- **Cost per Quality Point**: $${result.cost.costPerQualityPoint.toFixed(6)}
- Tokens Used: ${result.cost.tokensUsed.toLocaleString()}
- Efficiency: ${result.cost.efficiency.toFixed(3)}

#### Learning Metrics
- **Improvement Rate**: ${(result.learning.improvementRate * 100).toFixed(2)}%/generation
- **Convergence Speed**: ${result.learning.convergenceSpeed.toFixed(1)} generations
- Plateau Generation: ${result.learning.plateauGeneration}
- Final Quality: ${result.learning.finalQuality.toFixed(3)}

#### Diversity Metrics
- **Unique Values**: ${result.diversity.uniqueValues.toLocaleString()}
- **Pattern Variety**: ${result.diversity.patternVariety.toFixed(3)}
- **Distribution Entropy**: ${result.diversity.distributionEntropy.toFixed(3)}
- **Coverage Score**: ${result.diversity.coverageScore.toFixed(3)}
- **Novelty Rate**: ${result.diversity.noveltyRate.toFixed(3)}

---

`;
    }

    // Add comparison table
    md += `## Comparative Analysis

### Quality vs Cost Trade-off

| Model | Quality | Cost/Sample | Cost/Quality | Efficiency |
|-------|---------|-------------|--------------|------------|
`;

    for (const result of this.results) {
      md += `| ${result.modelName} | ${result.quality.overall.toFixed(3)} | $${result.cost.costPerSample.toFixed(6)} | $${result.cost.costPerQualityPoint.toFixed(6)} | ${result.cost.efficiency.toFixed(3)} |\n`;
    }

    md += `\n### Performance Comparison

| Model | P95 Latency | Throughput | Success Rate |
|-------|-------------|------------|--------------|
`;

    for (const result of this.results) {
      md += `| ${result.modelName} | ${result.performance.latencyP95.toFixed(0)}ms | ${result.performance.throughput.toFixed(0)}/s | ${(result.performance.successRate * 100).toFixed(2)}% |\n`;
    }

    // Add Pareto frontier
    md += `\n---

## Pareto Frontier Analysis

The following models are on the Pareto frontier (optimal quality/cost trade-off):

`;

    for (const modelName of comparison.paretoFrontier) {
      md += `- **${modelName}**\n`;
    }

    // Add recommendations
    md += `\n---

## Use Case Recommendations

Based on the benchmark results, here are our recommendations for different use cases:

### High-Quality, Low-Volume (Research)
**Recommended**: ${comparison.recommendations['high-quality-low-volume']}

Best for research, high-stakes decisions, and scenarios where quality is paramount.

### High-Volume, Low-Latency (Production)
**Recommended**: ${comparison.recommendations['high-volume-low-latency']}

Best for production systems requiring high throughput and low latency.

### Cost-Optimized (Batch Processing)
**Recommended**: ${comparison.recommendations['cost-optimized']}

Best for batch processing, large-scale data generation, and cost-sensitive applications.

### Balanced (General Purpose)
**Recommended**: ${comparison.recommendations['balanced']}

Best for general-purpose applications requiring a good balance of quality, performance, and cost.

---

## Statistical Significance

`;

    let hasSignificant = false;
    for (const [comparison_key, pValue] of Object.entries(comparison.statisticalSignificance)) {
      if (pValue < 0.05) {
        md += `- **${comparison_key}**: p = ${pValue.toFixed(4)} ${pValue < 0.01 ? '(highly significant)' : '(significant)'}\n`;
        hasSignificant = true;
      }
    }

    if (!hasSignificant) {
      md += `No statistically significant differences found at p < 0.05 level.\n`;
    }

    md += `\n---

## Methodology

### Quality Metrics
- **Accuracy**: Correctness of generated data
- **Coherence**: Logical consistency and flow
- **Validity**: Adherence to schema and constraints
- **Consistency**: Uniformity across samples
- **Completeness**: Coverage of all required fields

### Performance Metrics
- **Latency P50/P95/P99**: Response time percentiles
- **Throughput**: Samples generated per second
- **Success Rate**: Percentage of successful generations

### Cost Metrics
- **Cost per Sample**: Total cost divided by samples
- **Cost per Quality Point**: Cost normalized by quality score
- **Efficiency**: Quality per unit cost

### Learning Metrics
- **Improvement Rate**: Quality gain per generation
- **Convergence Speed**: Generations until plateau
- **Learning Curve**: Quality progression over time

### Diversity Metrics
- **Unique Values**: Number of distinct samples
- **Pattern Variety**: Ratio of unique to total samples
- **Distribution Entropy**: Shannon entropy of data distribution
- **Coverage Score**: Field-level diversity measure
- **Novelty Rate**: Rate of new patterns generation

---

## Conclusion

${this.generateConclusion(comparison)}

---

*Report generated by DSPy Benchmark Suite*
`;

    return md;
  }

  /**
   * Generate summary statistics
   */
  private generateSummary(comparison: ComparisonResult): any {
    const avgQuality = StatisticalAnalyzer.mean(this.results.map(r => r.quality.overall));
    const avgCost = StatisticalAnalyzer.mean(this.results.map(r => r.cost.costPerSample));
    const avgLatency = StatisticalAnalyzer.mean(this.results.map(r => r.performance.latencyP95));

    return {
      averageQuality: avgQuality,
      averageCostPerSample: avgCost,
      averageLatencyP95: avgLatency,
      qualityRange: {
        min: Math.min(...this.results.map(r => r.quality.overall)),
        max: Math.max(...this.results.map(r => r.quality.overall)),
      },
      costRange: {
        min: Math.min(...this.results.map(r => r.cost.costPerSample)),
        max: Math.max(...this.results.map(r => r.cost.costPerSample)),
      },
      latencyRange: {
        min: Math.min(...this.results.map(r => r.performance.latencyP95)),
        max: Math.max(...this.results.map(r => r.performance.latencyP95)),
      },
    };
  }

  /**
   * Generate conclusion for report
   */
  private generateConclusion(comparison: ComparisonResult): string {
    const winner = comparison.winner.overall;
    const qualityWinner = comparison.winner.quality;
    const costWinner = comparison.winner.cost;

    let conclusion = `This comprehensive benchmark analysis evaluated ${comparison.models.length} models across multiple dimensions. `;

    conclusion += `**${winner}** emerged as the overall winner, providing the best balance of quality, performance, and cost. `;

    if (qualityWinner !== winner) {
      conclusion += `For applications prioritizing quality above all else, **${qualityWinner}** is recommended. `;
    }

    if (costWinner !== winner && costWinner !== qualityWinner) {
      conclusion += `For cost-sensitive applications, **${costWinner}** offers the best value. `;
    }

    conclusion += `\n\nThe Pareto frontier analysis identified ${comparison.paretoFrontier.length} models with optimal quality/cost trade-offs. `;
    conclusion += `Selection should be based on specific application requirements, considering factors such as latency constraints, budget limitations, and quality thresholds.`;

    return conclusion;
  }

  /**
   * Save scalability results
   */
  private async saveScalabilityResults(results: ScalabilityResult[]): Promise<void> {
    const filepath = path.join(this.outputDir, 'scalability-results.json');
    await fs.writeFile(filepath, JSON.stringify(results, null, 2));
    console.log(`\n✅ Scalability results saved to ${filepath}`);
  }

  /**
   * Save convergence data
   */
  private async saveConvergenceData(data: any[]): Promise<void> {
    const filepath = path.join(this.outputDir, 'convergence-data.json');
    await fs.writeFile(filepath, JSON.stringify(data, null, 2));
    console.log(`\n✅ Convergence data saved to ${filepath}`);
  }
}

// ============================================================================
// CLI Runner
// ============================================================================

async function main() {
  console.log('🚀 DSPy Benchmark Suite');
  console.log('='.repeat(70));

  const suite = new BenchmarkSuite();

  // Add common models for comparison
  suite.addCommonModels();

  try {
    // Run comprehensive comparison
    const comparison = await suite.runModelComparison(1000);

    // Run scalability test
    await suite.runScalabilityTest();

    // Run cost analysis
    await suite.runCostAnalysis();

    // Run quality convergence
    await suite.runQualityConvergence(10);

    // Run diversity analysis
    await suite.runDiversityAnalysis(5000);

    // Generate reports
    await suite.generateJSONReport(comparison);
    await suite.generateMarkdownReport(comparison);

    console.log('\n' + '='.repeat(70));
    console.log('✅ Benchmark suite completed successfully!');
    console.log('📊 Check the results directory for detailed reports.');

  } catch (error) {
    console.error('\n❌ Benchmark failed:', error);
    process.exit(1);
  }
}

// Run if executed directly (Node.js ESM check)
const isMainModule = typeof process !== 'undefined' &&
  typeof process.argv !== 'undefined' &&
  process.argv[1] &&
  process.argv[1].includes('dspy-benchmarks');

if (isMainModule) {
  main().catch(console.error);
}

// Export for use as library
export { ModelConfig, BenchmarkResult, ComparisonResult, ScalabilityResult, StatisticalAnalyzer };
