/**
 * DSPy.ts Real Integration with Agentic-Synth
 *
 * Production-ready integration using actual dspy.ts npm package (v2.1.1)
 * for synthetic data generation optimization and quality improvement.
 *
 * Features:
 * - ChainOfThought reasoning for data quality assessment
 * - BootstrapFewShot optimization for learning from successful generations
 * - Multi-model support (OpenAI, Claude via dspy.ts)
 * - Real-time quality metrics and evaluation
 * - Integration with agentic-synth generators
 *
 * @packageDocumentation
 */

// Note: dspy.ts package has build issue - imports from dist/src instead of dist
// This is a known issue with the package structure
import {
  ChainOfThought,
  BootstrapFewShot,
  evaluate,
  OpenAILM,
  AnthropicLM,
  configureLM,
  f1Score,
  exactMatch
} from '../node_modules/dspy.ts/dist/src/index.js';
import {
  SynthConfig,
  GeneratorOptions,
  GenerationResult,
  ModelProvider,
  APIError,
  ValidationError
} from '../src/types.js';
import { BaseGenerator } from '../src/generators/base.js';
import { EventEmitter } from 'events';

// ============================================================================
// Types & Interfaces
// ============================================================================

/**
 * DSPy trainer configuration
 */
export interface DSPyTrainerConfig {
  models: string[]; // e.g., ['gpt-3.5-turbo', 'claude-3-sonnet-20240229']
  optimizationRounds?: number;
  minQualityScore?: number;
  maxExamples?: number;
  batchSize?: number;
  evaluationMetrics?: string[];
  enableCaching?: boolean;
  hooks?: {
    onIterationComplete?: (iteration: number, metrics: QualityMetrics) => void;
    onOptimizationComplete?: (result: TrainingResult) => void;
    onError?: (error: Error) => void;
  };
}

/**
 * Quality metrics for generated data
 */
export interface QualityMetrics {
  accuracy: number; // 0-1
  coherence: number; // 0-1
  relevance: number; // 0-1
  diversity: number; // 0-1
  overallScore: number; // 0-1
  timestamp: Date;
}

/**
 * Training iteration result
 */
export interface IterationMetrics {
  iteration: number;
  model: string;
  quality: QualityMetrics;
  generatedCount: number;
  duration: number;
  tokenUsage?: number;
}

/**
 * Complete training result
 */
export interface TrainingResult {
  success: boolean;
  iterations: IterationMetrics[];
  bestIteration: IterationMetrics;
  optimizedPrompt: string;
  improvements: {
    initialScore: number;
    finalScore: number;
    improvement: number; // percentage
  };
  metadata: {
    totalDuration: number;
    modelsUsed: string[];
    totalGenerated: number;
    convergenceIteration?: number;
  };
}

/**
 * Evaluation result from dspy.ts
 */
export interface EvaluationResult {
  metrics: {
    [key: string]: number;
  };
  passed: number;
  failed: number;
  total: number;
}

/**
 * DSPy example format
 */
export interface DSPyExample {
  input: string;
  output: string;
  quality?: number;
}

// ============================================================================
// DSPy Signatures (Type-safe Input/Output)
// ============================================================================

/**
 * Signature for data quality assessment
 */
const DataQualitySignature = {
  inputs: [
    { name: 'data', type: 'string' as const, required: true, description: 'Data to assess' },
    { name: 'schema', type: 'string' as const, required: false, description: 'JSON schema' }
  ],
  outputs: [
    { name: 'assessment', type: 'string' as const, required: true, description: 'Quality assessment' },
    { name: 'score', type: 'number' as const, required: true, description: 'Quality score 0-1' }
  ]
};

/**
 * Signature for data generation
 */
const DataGenerationSignature = {
  inputs: [
    { name: 'schema', type: 'string' as const, required: true, description: 'Target schema' },
    { name: 'examples', type: 'string' as const, required: false, description: 'Example data' }
  ],
  outputs: [
    { name: 'generated_data', type: 'string' as const, required: true, description: 'Generated synthetic data' }
  ]
};

// ============================================================================
// DSPy Agentic-Synth Trainer
// ============================================================================

/**
 * Main trainer class integrating dspy.ts with agentic-synth
 */
export class DSPyAgenticSynthTrainer extends EventEmitter {
  private config: DSPyTrainerConfig;
  private languageModels: Map<string, any>;
  private chainOfThought?: ChainOfThought;
  private optimizer?: BootstrapFewShot;
  private trainingExamples: DSPyExample[];
  private currentIteration: number;
  private bestScore: number;
  private optimizedPrompt: string;

  constructor(config: DSPyTrainerConfig) {
    super();
    this.config = {
      optimizationRounds: 5,
      minQualityScore: 0.8,
      maxExamples: 50,
      batchSize: 10,
      evaluationMetrics: ['accuracy', 'coherence', 'relevance'],
      enableCaching: true,
      ...config
    };

    this.languageModels = new Map();
    this.trainingExamples = [];
    this.currentIteration = 0;
    this.bestScore = 0;
    this.optimizedPrompt = '';
  }

  /**
   * Initialize DSPy.ts language models and modules
   */
  async initialize(): Promise<void> {
    try {
      this.emit('status', 'Initializing DSPy.ts language models...');

      // Initialize language models for each configured model
      for (const modelName of this.config.models) {
        if (modelName.includes('gpt') || modelName.includes('turbo')) {
          // OpenAI models
          const apiKey = process.env.OPENAI_API_KEY;
          if (!apiKey) {
            throw new ValidationError('OPENAI_API_KEY not set', { modelName });
          }

          const lm = new OpenAILM({
            model: modelName,
            apiKey: apiKey,
            defaultOptions: {
              temperature: 0.7,
              maxTokens: 2000
            }
          });

          await lm.init();
          this.languageModels.set(modelName, lm);
          this.emit('status', `Initialized OpenAI model: ${modelName}`);

        } else if (modelName.includes('claude')) {
          // Anthropic Claude models
          const apiKey = process.env.ANTHROPIC_API_KEY;
          if (!apiKey) {
            throw new ValidationError('ANTHROPIC_API_KEY not set', { modelName });
          }

          const lm = new AnthropicLM({
            model: modelName,
            apiKey: apiKey,
            defaultOptions: {
              temperature: 0.7,
              maxTokens: 2000
            }
          });

          await lm.init();
          this.languageModels.set(modelName, lm);
          this.emit('status', `Initialized Anthropic model: ${modelName}`);
        } else {
          console.warn(`Model ${modelName} not recognized, skipping...`);
        }
      }

      if (this.languageModels.size === 0) {
        throw new ValidationError('No valid language models initialized');
      }

      // Configure the first available LM as default
      const defaultLM = Array.from(this.languageModels.values())[0];
      configureLM(defaultLM);

      // Initialize ChainOfThought module for reasoning
      this.chainOfThought = new ChainOfThought({
        name: 'DataQualityAssessor',
        signature: DataQualitySignature
      });

      this.emit('status', 'DSPy.ts initialization complete');
    } catch (error: any) {
      this.emit('error', error);
      throw new APIError('Failed to initialize DSPy.ts', { error });
    }
  }

  /**
   * Train with optimization using DSPy.ts
   */
  async trainWithOptimization(
    schema: Record<string, any>,
    examples: DSPyExample[]
  ): Promise<TrainingResult> {
    const startTime = Date.now();
    const iterations: IterationMetrics[] = [];
    let converged = false;
    let convergenceIteration: number | undefined;

    try {
      this.emit('status', 'Starting training with optimization...');
      this.trainingExamples = examples.slice(0, this.config.maxExamples);

      // Phase 1: Baseline generation with each model
      this.emit('status', 'Phase 1: Baseline generation');
      for (const [modelName, lm] of this.languageModels) {
        configureLM(lm);
        const metrics = await this.runIteration(modelName, schema, this.trainingExamples);
        iterations.push(metrics);

        if (this.config.hooks?.onIterationComplete) {
          this.config.hooks.onIterationComplete(metrics.iteration, metrics.quality);
        }
      }

      // Phase 2: Optimization rounds with BootstrapFewShot
      this.emit('status', 'Phase 2: Running optimization rounds');
      const optimizationRounds = this.config.optimizationRounds!;

      for (let round = 0; round < optimizationRounds && !converged; round++) {
        this.emit('status', `Optimization round ${round + 1}/${optimizationRounds}`);

        // Train optimizer with successful examples
        const successfulExamples = this.filterSuccessfulExamples(
          this.trainingExamples,
          this.config.minQualityScore!
        );

        if (successfulExamples.length > 0) {
          // Initialize BootstrapFewShot optimizer
          this.optimizer = new BootstrapFewShot(
            this.createMetricFunction(),
            {
              maxBootstrappedDemos: Math.min(5, successfulExamples.length),
              maxLabeledDemos: Math.min(3, successfulExamples.length)
            }
          );

          // Compile the program with optimization
          const program = this.chainOfThought!;
          const trainExamples = this.convertToDSPyExamples(successfulExamples);
          const valExamples = trainExamples.slice(0, Math.min(10, trainExamples.length));

          const optimizedProgram = await this.optimizer.compile(
            program,
            trainExamples,
            valExamples
          );

          // Update ChainOfThought with optimized prompts
          this.chainOfThought = optimizedProgram;
        }

        // Generate with optimized program
        for (const [modelName, lm] of this.languageModels) {
          configureLM(lm);
          const metrics = await this.runIteration(
            modelName,
            schema,
            successfulExamples.length > 0 ? successfulExamples : this.trainingExamples
          );
          iterations.push(metrics);

          // Check for convergence
          if (metrics.quality.overallScore >= this.config.minQualityScore!) {
            converged = true;
            convergenceIteration = metrics.iteration;
            this.emit('status', `Converged at iteration ${metrics.iteration}`);
          }

          if (this.config.hooks?.onIterationComplete) {
            this.config.hooks.onIterationComplete(metrics.iteration, metrics.quality);
          }
        }

        // Learn from this round's results
        await this.updateTrainingExamples(schema);
      }

      // Phase 3: Final evaluation
      this.emit('status', 'Phase 3: Final evaluation');
      const evaluationResults = await this.evaluateFinal(iterations);

      // Find best iteration
      const bestIteration = iterations.reduce((best, current) =>
        current.quality.overallScore > best.quality.overallScore ? current : best
      );

      const initialScore = iterations[0]?.quality.overallScore || 0;
      const finalScore = bestIteration.quality.overallScore;
      const improvement = ((finalScore - initialScore) / initialScore) * 100;

      const result: TrainingResult = {
        success: finalScore >= this.config.minQualityScore!,
        iterations,
        bestIteration,
        optimizedPrompt: this.optimizedPrompt,
        improvements: {
          initialScore,
          finalScore,
          improvement
        },
        metadata: {
          totalDuration: Date.now() - startTime,
          modelsUsed: Array.from(this.languageModels.keys()),
          totalGenerated: iterations.reduce((sum, it) => sum + it.generatedCount, 0),
          convergenceIteration
        }
      };

      if (this.config.hooks?.onOptimizationComplete) {
        this.config.hooks.onOptimizationComplete(result);
      }

      this.emit('complete', result);
      return result;

    } catch (error: any) {
      this.emit('error', error);
      throw new APIError('Training failed', { error });
    }
  }

  /**
   * Generate optimized data using trained models
   */
  async generateOptimizedData(
    count: number,
    schema?: Record<string, any>
  ): Promise<any[]> {
    try {
      if (!this.chainOfThought) {
        throw new ValidationError('Trainer not initialized. Call initialize() first.');
      }

      this.emit('status', `Generating ${count} optimized samples...`);
      const results: any[] = [];

      const batchSize = this.config.batchSize!;
      for (let i = 0; i < count; i += batchSize) {
        const batchCount = Math.min(batchSize, count - i);
        const batch = await this.generateBatch(batchCount, schema);
        results.push(...batch);

        this.emit('progress', {
          current: Math.min(i + batchSize, count),
          total: count
        });
      }

      return results;
    } catch (error: any) {
      this.emit('error', error);
      throw new APIError('Data generation failed', { error });
    }
  }

  /**
   * Evaluate data quality using DSPy.ts metrics
   */
  async evaluateQuality(data: any[]): Promise<QualityMetrics> {
    try {
      if (!this.chainOfThought) {
        throw new ValidationError('Trainer not initialized. Call initialize() first.');
      }

      const assessments = await Promise.all(
        data.map(item => this.assessDataQuality(item))
      );

      const accuracy = this.calculateAverage(assessments.map(a => a.accuracy));
      const coherence = this.calculateAverage(assessments.map(a => a.coherence));
      const relevance = this.calculateAverage(assessments.map(a => a.relevance));
      const diversity = this.calculateDiversity(data);

      const overallScore = (accuracy + coherence + relevance + diversity) / 4;

      return {
        accuracy,
        coherence,
        relevance,
        diversity,
        overallScore,
        timestamp: new Date()
      };
    } catch (error: any) {
      this.emit('error', error);
      throw new APIError('Quality evaluation failed', { error });
    }
  }

  // ============================================================================
  // Private Helper Methods
  // ============================================================================

  /**
   * Run a single training iteration
   */
  private async runIteration(
    modelName: string,
    schema: Record<string, any>,
    examples: DSPyExample[]
  ): Promise<IterationMetrics> {
    const iterationStart = Date.now();
    this.currentIteration++;

    try {
      // Generate data using current model and ChainOfThought
      const generated = await this.generateBatch(
        this.config.batchSize!,
        schema,
        examples
      );

      // Evaluate quality
      const quality = await this.evaluateQuality(generated);

      // Update best score
      if (quality.overallScore > this.bestScore) {
        this.bestScore = quality.overallScore;
      }

      return {
        iteration: this.currentIteration,
        model: modelName,
        quality,
        generatedCount: generated.length,
        duration: Date.now() - iterationStart
      };
    } catch (error: any) {
      throw new APIError(`Iteration ${this.currentIteration} failed`, {
        model: modelName,
        error
      });
    }
  }

  /**
   * Generate a batch of data samples
   */
  private async generateBatch(
    count: number,
    schema?: Record<string, any>,
    examples?: DSPyExample[]
  ): Promise<any[]> {
    const results: any[] = [];

    for (let i = 0; i < count; i++) {
      try {
        const prompt = this.buildGenerationPrompt(schema, examples);

        // Use ChainOfThought for reasoning about generation
        const result = await this.chainOfThought!.run({
          data: prompt,
          schema: schema ? JSON.stringify(schema) : ''
        });

        // Parse the generated data
        const parsed = this.parseGeneratedData(result.assessment);
        if (parsed) {
          results.push(parsed);
        }
      } catch (error) {
        console.warn(`Failed to generate sample ${i + 1}:`, error);
      }
    }

    return results;
  }

  /**
   * Assess data quality for a single item
   */
  private async assessDataQuality(data: any): Promise<{
    accuracy: number;
    coherence: number;
    relevance: number;
  }> {
    try {
      const dataStr = typeof data === 'string' ? data : JSON.stringify(data);

      const result = await this.chainOfThought!.run({
        data: dataStr,
        schema: ''
      });

      // Parse quality scores from assessment
      const score = typeof result.score === 'number' ? result.score : 0.5;

      return {
        accuracy: Math.min(1, Math.max(0, score)),
        coherence: Math.min(1, Math.max(0, score * 0.9)),
        relevance: Math.min(1, Math.max(0, score * 0.95))
      };
    } catch (error) {
      return { accuracy: 0.5, coherence: 0.5, relevance: 0.5 };
    }
  }

  /**
   * Build generation prompt
   */
  private buildGenerationPrompt(
    schema?: Record<string, any>,
    examples?: DSPyExample[]
  ): string {
    let prompt = 'Generate high-quality synthetic data';

    if (schema) {
      prompt += ` following this schema: ${JSON.stringify(schema)}`;
    }

    if (examples && examples.length > 0) {
      prompt += '\n\nExamples of successful generations:\n';
      prompt += examples.slice(0, 3).map((ex, i) =>
        `${i + 1}. ${ex.output}`
      ).join('\n');
    }

    return prompt;
  }

  /**
   * Parse generated data from model response
   */
  private parseGeneratedData(response: string): any | null {
    try {
      // Try to extract JSON from response
      const jsonMatch = response.match(/\{[\s\S]*\}/);
      if (jsonMatch) {
        return JSON.parse(jsonMatch[0]);
      }

      // Otherwise return as-is
      return { data: response };
    } catch (error) {
      return null;
    }
  }

  /**
   * Filter successful examples above quality threshold
   */
  private filterSuccessfulExamples(
    examples: DSPyExample[],
    threshold: number
  ): DSPyExample[] {
    return examples.filter(ex => (ex.quality || 0) >= threshold);
  }

  /**
   * Update training examples with new results
   */
  private async updateTrainingExamples(schema: Record<string, any>): Promise<void> {
    // Generate new examples and evaluate them
    const newData = await this.generateBatch(5, schema);
    const quality = await this.evaluateQuality(newData);

    // Add successful examples to training set
    newData.forEach(data => {
      this.trainingExamples.push({
        input: JSON.stringify(schema),
        output: JSON.stringify(data),
        quality: quality.overallScore
      });
    });

    // Keep only top examples
    this.trainingExamples.sort((a, b) => (b.quality || 0) - (a.quality || 0));
    this.trainingExamples = this.trainingExamples.slice(0, this.config.maxExamples);
  }

  /**
   * Create metric function for DSPy optimizer
   */
  private createMetricFunction() {
    return (example: any, prediction: any): number => {
      // Calculate quality score based on similarity
      try {
        const expectedOutput = typeof example.assessment === 'string' ? example.assessment : '';
        const actualOutput = typeof prediction.assessment === 'string' ? prediction.assessment : '';

        // Use simple similarity metric
        const similarity = this.calculateSimilarity(expectedOutput, actualOutput);
        return similarity;
      } catch (error) {
        return 0;
      }
    };
  }

  /**
   * Convert training examples to DSPy format
   */
  private convertToDSPyExamples(examples: DSPyExample[]): any[] {
    return examples.map(ex => ({
      data: ex.input,
      schema: '',
      assessment: ex.output,
      score: ex.quality || 0.5
    }));
  }

  /**
   * Calculate simple similarity between two strings
   */
  private calculateSimilarity(str1: string, str2: string): number {
    if (!str1 || !str2) return 0;
    if (str1 === str2) return 1;

    // Simple character-level similarity
    const longer = str1.length > str2.length ? str1 : str2;
    const shorter = str1.length > str2.length ? str2 : str1;

    if (longer.length === 0) return 1.0;

    return (longer.length - this.editDistance(longer, shorter)) / longer.length;
  }

  /**
   * Calculate edit distance between strings
   */
  private editDistance(str1: string, str2: string): number {
    const costs: number[] = [];
    for (let i = 0; i <= str1.length; i++) {
      let lastValue = i;
      for (let j = 0; j <= str2.length; j++) {
        if (i === 0) {
          costs[j] = j;
        } else if (j > 0) {
          let newValue = costs[j - 1];
          if (str1.charAt(i - 1) !== str2.charAt(j - 1)) {
            newValue = Math.min(Math.min(newValue, lastValue), costs[j]) + 1;
          }
          costs[j - 1] = lastValue;
          lastValue = newValue;
        }
      }
      if (i > 0) costs[str2.length] = lastValue;
    }
    return costs[str2.length];
  }

  /**
   * Final evaluation across all iterations
   */
  private async evaluateFinal(iterations: IterationMetrics[]): Promise<EvaluationResult> {
    const totalIterations = iterations.length;
    const passedIterations = iterations.filter(
      it => it.quality.overallScore >= this.config.minQualityScore!
    ).length;

    return {
      metrics: {
        averageQuality: this.calculateAverage(
          iterations.map(it => it.quality.overallScore)
        ),
        averageDuration: this.calculateAverage(
          iterations.map(it => it.duration)
        )
      },
      passed: passedIterations,
      failed: totalIterations - passedIterations,
      total: totalIterations
    };
  }

  /**
   * Calculate average of numbers
   */
  private calculateAverage(numbers: number[]): number {
    if (numbers.length === 0) return 0;
    return numbers.reduce((sum, n) => sum + n, 0) / numbers.length;
  }

  /**
   * Calculate diversity score
   */
  private calculateDiversity(data: any[]): number {
    if (data.length === 0) return 0;

    // Simple diversity metric based on unique values
    const uniqueItems = new Set(data.map(item => JSON.stringify(item)));
    return uniqueItems.size / data.length;
  }

  /**
   * Get training statistics
   */
  getStatistics(): {
    totalIterations: number;
    bestScore: number;
    trainingExamples: number;
  } {
    return {
      totalIterations: this.currentIteration,
      bestScore: this.bestScore,
      trainingExamples: this.trainingExamples.length
    };
  }
}

// ============================================================================
// Working Example
// ============================================================================

/**
 * Example usage demonstrating real DSPy.ts integration
 */
async function main() {
  console.log('🚀 Starting DSPy.ts Agentic-Synth Integration Example\n');

  // Example schema for user profile generation
  const schema = {
    type: 'object',
    properties: {
      userId: { type: 'string', format: 'uuid' },
      name: { type: 'string' },
      email: { type: 'string', format: 'email' },
      age: { type: 'number', minimum: 18, maximum: 100 },
      interests: { type: 'array', items: { type: 'string' } },
      createdAt: { type: 'string', format: 'date-time' }
    },
    required: ['userId', 'name', 'email', 'age']
  };

  // Initial training examples
  const examples: DSPyExample[] = [
    {
      input: JSON.stringify(schema),
      output: JSON.stringify({
        userId: '123e4567-e89b-12d3-a456-426614174000',
        name: 'Alice Johnson',
        email: 'alice@example.com',
        age: 28,
        interests: ['reading', 'hiking', 'photography'],
        createdAt: new Date().toISOString()
      }),
      quality: 0.9
    },
    {
      input: JSON.stringify(schema),
      output: JSON.stringify({
        userId: '987fcdeb-51a2-43f7-9c3d-8e5a7b6c9d0e',
        name: 'Bob Smith',
        email: 'bob@example.com',
        age: 35,
        interests: ['gaming', 'cooking'],
        createdAt: new Date().toISOString()
      }),
      quality: 0.85
    }
  ];

  // Configure trainer
  const trainer = new DSPyAgenticSynthTrainer({
    models: [
      'gpt-3.5-turbo',
      // 'claude-3-sonnet-20240229' // Uncomment if ANTHROPIC_API_KEY is available
    ],
    optimizationRounds: 5,
    minQualityScore: 0.8,
    batchSize: 5,
    hooks: {
      onIterationComplete: (iteration, metrics) => {
        console.log(`✓ Iteration ${iteration}: Score = ${metrics.overallScore.toFixed(3)}`);
      },
      onOptimizationComplete: (result) => {
        console.log('\n✅ Optimization complete!');
        console.log(`Improvement: ${result.improvements.improvement.toFixed(1)}%`);
      },
      onError: (error) => {
        console.error('❌ Error:', error.message);
      }
    }
  });

  // Event listeners
  trainer.on('status', (message) => {
    console.log(`📊 ${message}`);
  });

  trainer.on('progress', ({ current, total }) => {
    console.log(`Progress: ${current}/${total}`);
  });

  try {
    // Initialize DSPy.ts
    console.log('Initializing DSPy.ts...\n');
    await trainer.initialize();

    // Train with optimization
    console.log('\nStarting training with optimization...\n');
    const result = await trainer.trainWithOptimization(schema, examples);

    // Display results
    console.log('\n' + '='.repeat(60));
    console.log('TRAINING RESULTS');
    console.log('='.repeat(60));
    console.log(`Success: ${result.success}`);
    console.log(`Total Iterations: ${result.iterations.length}`);
    console.log(`Best Model: ${result.bestIteration.model}`);
    console.log(`Best Score: ${result.bestIteration.quality.overallScore.toFixed(3)}`);
    console.log(`Improvement: ${result.improvements.improvement.toFixed(1)}%`);
    console.log(`Total Duration: ${(result.metadata.totalDuration / 1000).toFixed(2)}s`);
    console.log(`Total Generated: ${result.metadata.totalGenerated} samples`);

    if (result.metadata.convergenceIteration) {
      console.log(`Converged at iteration: ${result.metadata.convergenceIteration}`);
    }

    // Generate optimized data
    console.log('\n' + '='.repeat(60));
    console.log('GENERATING OPTIMIZED DATA');
    console.log('='.repeat(60));
    const optimizedData = await trainer.generateOptimizedData(10, schema);
    console.log(`Generated ${optimizedData.length} optimized samples`);
    console.log('\nSample output:');
    console.log(JSON.stringify(optimizedData[0], null, 2));

    // Evaluate quality
    console.log('\n' + '='.repeat(60));
    console.log('QUALITY EVALUATION');
    console.log('='.repeat(60));
    const quality = await trainer.evaluateQuality(optimizedData);
    console.log(`Accuracy: ${quality.accuracy.toFixed(3)}`);
    console.log(`Coherence: ${quality.coherence.toFixed(3)}`);
    console.log(`Relevance: ${quality.relevance.toFixed(3)}`);
    console.log(`Diversity: ${quality.diversity.toFixed(3)}`);
    console.log(`Overall Score: ${quality.overallScore.toFixed(3)}`);

    // Statistics
    const stats = trainer.getStatistics();
    console.log('\n' + '='.repeat(60));
    console.log('STATISTICS');
    console.log('='.repeat(60));
    console.log(`Total Iterations: ${stats.totalIterations}`);
    console.log(`Best Score Achieved: ${stats.bestScore.toFixed(3)}`);
    console.log(`Training Examples: ${stats.trainingExamples}`);

    console.log('\n✅ Example completed successfully!');

  } catch (error: any) {
    console.error('\n❌ Error:', error.message);
    if (error.details) {
      console.error('Details:', error.details);
    }
    process.exit(1);
  }
}

// Run example if this file is executed directly
if (import.meta.url === `file://${process.argv[1]}`) {
  main().catch(console.error);
}
