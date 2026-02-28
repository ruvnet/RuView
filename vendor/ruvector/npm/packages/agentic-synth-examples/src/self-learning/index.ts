/**
 * Self-Learning Generator - Adaptive data generation with feedback loops
 *
 * This generator improves its output quality over time by learning from feedback
 * and tracking performance metrics. It demonstrates how synthetic data generation
 * can evolve and adapt based on usage patterns and quality assessments.
 *
 * @packageDocumentation
 */

import { EventEmitter } from 'events';
import { AgenticSynth, SynthConfig, GenerationResult, GeneratorOptions } from '@ruvector/agentic-synth';

/**
 * Feedback data structure for learning improvements
 */
export interface FeedbackData {
  generationId: string;
  quality: number; // 0-1 score
  timestamp: Date;
  corrections?: Record<string, unknown>;
  comments?: string;
}

/**
 * Learning metrics tracking improvements over time
 */
export interface LearningMetrics {
  totalGenerations: number;
  averageQuality: number;
  improvementRate: number;
  feedbackCount: number;
  lastUpdated: Date;
}

/**
 * Configuration for self-learning behavior
 */
export interface SelfLearningConfig extends Partial<SynthConfig> {
  learningRate?: number; // 0-1, how quickly to adapt
  qualityThreshold?: number; // Minimum acceptable quality score
  feedbackWindowSize?: number; // Number of recent feedbacks to consider
  autoAdapt?: boolean; // Enable automatic adaptation
}

/**
 * Generation history entry
 */
interface GenerationHistory {
  id: string;
  timestamp: Date;
  options: GeneratorOptions;
  result: GenerationResult;
  feedback?: FeedbackData;
}

/**
 * Self-Learning Generator with adaptive improvement
 *
 * Features:
 * - Tracks generation quality over time
 * - Learns from user feedback
 * - Adapts prompts and parameters based on performance
 * - Emits progress events for monitoring
 *
 * @example
 * ```typescript
 * const generator = new SelfLearningGenerator({
 *   provider: 'gemini',
 *   apiKey: process.env.GEMINI_API_KEY,
 *   learningRate: 0.3,
 *   autoAdapt: true
 * });
 *
 * // Generate with learning
 * const result = await generator.generateWithLearning({
 *   count: 10,
 *   schema: { name: { type: 'string' }, age: { type: 'number' } }
 * });
 *
 * // Provide feedback
 * await generator.provideFeedback(result.metadata.generationId, {
 *   quality: 0.85,
 *   comments: 'Good quality, names are realistic'
 * });
 *
 * // Get metrics
 * const metrics = generator.getMetrics();
 * console.log(`Average quality: ${metrics.averageQuality}`);
 * ```
 */
export class SelfLearningGenerator extends EventEmitter {
  private synth: AgenticSynth;
  private config: SelfLearningConfig;
  private history: GenerationHistory[] = [];
  private metrics: LearningMetrics;
  private feedbackBuffer: FeedbackData[] = [];

  constructor(config: SelfLearningConfig = {}) {
    super();

    // Set defaults
    this.config = {
      provider: config.provider || 'gemini',
      apiKey: config.apiKey || process.env.GEMINI_API_KEY || '',
      ...(config.model && { model: config.model }),
      cacheStrategy: config.cacheStrategy || 'memory',
      cacheTTL: config.cacheTTL || 3600,
      maxRetries: config.maxRetries || 3,
      timeout: config.timeout || 30000,
      streaming: config.streaming || false,
      automation: config.automation || false,
      vectorDB: config.vectorDB || false,
      learningRate: config.learningRate ?? 0.2,
      qualityThreshold: config.qualityThreshold ?? 0.7,
      feedbackWindowSize: config.feedbackWindowSize ?? 50,
      autoAdapt: config.autoAdapt ?? true
    };

    this.synth = new AgenticSynth(this.config);

    this.metrics = {
      totalGenerations: 0,
      averageQuality: 0,
      improvementRate: 0,
      feedbackCount: 0,
      lastUpdated: new Date()
    };
  }

  /**
   * Generate data with learning integration
   */
  async generateWithLearning<T = unknown>(
    options: GeneratorOptions
  ): Promise<GenerationResult<T> & { generationId: string }> {
    this.emit('generation:start', { options });

    try {
      // Adapt options based on learning
      const adaptedOptions = this.config.autoAdapt
        ? this.adaptOptions(options)
        : options;

      this.emit('generation:adapted', { original: options, adapted: adaptedOptions });

      // Generate data
      const result = await this.synth.generateStructured<T>(adaptedOptions);

      // Create history entry
      const generationId = this.generateId();
      const historyEntry: GenerationHistory = {
        id: generationId,
        timestamp: new Date(),
        options: adaptedOptions,
        result: result as any
      };

      this.history.push(historyEntry);
      this.metrics.totalGenerations++;
      this.metrics.lastUpdated = new Date();

      this.emit('generation:complete', {
        generationId,
        count: result.data.length,
        metrics: this.metrics
      });

      return { ...result, generationId };
    } catch (error) {
      this.emit('generation:error', { error, options });
      throw error;
    }
  }

  /**
   * Provide feedback for a generation to improve future outputs
   */
  async provideFeedback(generationId: string, feedback: Omit<FeedbackData, 'generationId' | 'timestamp'>): Promise<void> {
    const historyEntry = this.history.find(h => h.id === generationId);
    if (!historyEntry) {
      throw new Error(`Generation ${generationId} not found in history`);
    }

    const feedbackData: FeedbackData = {
      generationId,
      quality: feedback.quality,
      timestamp: new Date(),
      corrections: feedback.corrections,
      comments: feedback.comments
    };

    // Store feedback
    historyEntry.feedback = feedbackData;
    this.feedbackBuffer.push(feedbackData);

    // Trim buffer
    const maxSize = this.config.feedbackWindowSize ?? 50;
    if (this.feedbackBuffer.length > maxSize) {
      this.feedbackBuffer.shift();
    }

    // Update metrics
    this.updateMetrics();

    this.emit('feedback:received', {
      generationId,
      quality: feedback.quality,
      metrics: this.metrics
    });

    // Auto-adapt if enabled
    if (this.config.autoAdapt) {
      await this.adapt();
    }
  }

  /**
   * Adapt generation strategy based on feedback
   */
  private async adapt(): Promise<void> {
    if (this.feedbackBuffer.length < 5) {
      return; // Need minimum feedback samples
    }

    this.emit('adaptation:start', { feedbackCount: this.feedbackBuffer.length });

    // Analyze patterns in feedback
    const recentFeedback = this.feedbackBuffer.slice(-10);
    const avgQuality = recentFeedback.reduce((sum, f) => sum + f.quality, 0) / recentFeedback.length;

    // Check if below threshold
    const threshold = this.config.qualityThreshold ?? 0.7;
    const learningRate = this.config.learningRate ?? 0.2;
    if (avgQuality < threshold) {
      // Adjust learning parameters
      const adjustment = (threshold - avgQuality) * learningRate;

      this.emit('adaptation:adjusting', {
        avgQuality,
        threshold,
        adjustment
      });
    }

    this.emit('adaptation:complete', { metrics: this.metrics });
  }

  /**
   * Adapt generation options based on learning
   */
  private adaptOptions(options: GeneratorOptions): GeneratorOptions {
    if (this.feedbackBuffer.length === 0) {
      return options;
    }

    // Find patterns in successful generations
    const threshold = this.config.qualityThreshold ?? 0.7;
    const goodGenerations = this.history.filter(h =>
      h.feedback && h.feedback.quality >= threshold
    );

    if (goodGenerations.length === 0) {
      return options;
    }

    // Apply learned adjustments
    const adapted = { ...options };

    // Example: Adjust count based on quality feedback
    if (adapted.count && this.metrics.averageQuality > 0.8) {
      adapted.count = Math.ceil(adapted.count * 1.1); // Increase by 10%
    }

    return adapted;
  }

  /**
   * Update metrics based on feedback
   */
  private updateMetrics(): void {
    const withFeedback = this.history.filter(h => h.feedback);

    if (withFeedback.length === 0) {
      return;
    }

    const totalQuality = withFeedback.reduce((sum, h) =>
      sum + (h.feedback?.quality || 0), 0
    );

    const oldAvg = this.metrics.averageQuality;
    this.metrics.averageQuality = totalQuality / withFeedback.length;
    this.metrics.feedbackCount = withFeedback.length;
    this.metrics.improvementRate = this.metrics.averageQuality - oldAvg;
    this.metrics.lastUpdated = new Date();
  }

  /**
   * Get current learning metrics
   */
  getMetrics(): LearningMetrics {
    return { ...this.metrics };
  }

  /**
   * Get generation history
   */
  getHistory(limit?: number): GenerationHistory[] {
    const history = [...this.history].reverse();
    return limit ? history.slice(0, limit) : history;
  }

  /**
   * Reset learning state
   */
  reset(): void {
    this.history = [];
    this.feedbackBuffer = [];
    this.metrics = {
      totalGenerations: 0,
      averageQuality: 0,
      improvementRate: 0,
      feedbackCount: 0,
      lastUpdated: new Date()
    };

    this.emit('reset', { timestamp: new Date() });
  }

  /**
   * Export learning data for persistence
   */
  export(): { config: SelfLearningConfig; metrics: LearningMetrics; historyCount: number } {
    return {
      config: this.config,
      metrics: this.metrics,
      historyCount: this.history.length
    };
  }

  /**
   * Generate unique ID for tracking
   */
  private generateId(): string {
    return `gen_${Date.now()}_${Math.random().toString(36).substring(2, 9)}`;
  }
}

/**
 * Create a new self-learning generator instance
 */
export function createSelfLearningGenerator(config?: SelfLearningConfig): SelfLearningGenerator {
  return new SelfLearningGenerator(config);
}
