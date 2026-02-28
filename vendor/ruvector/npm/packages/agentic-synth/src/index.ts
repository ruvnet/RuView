/**
 * agentic-synth - AI-powered synthetic data generation
 *
 * @packageDocumentation
 */

import 'dotenv/config';
import {
  SynthConfig,
  SynthConfigSchema,
  GeneratorOptions,
  TimeSeriesOptions,
  EventOptions,
  GenerationResult,
  ModelProvider,
  DataType
} from './types.js';
import { TimeSeriesGenerator } from './generators/timeseries.js';
import { EventGenerator } from './generators/events.js';
import { StructuredGenerator } from './generators/structured.js';
import { CacheManager } from './cache/index.js';

/**
 * Main AgenticSynth class for data generation
 */
export class AgenticSynth {
  private config: SynthConfig;
  private timeSeriesGen: TimeSeriesGenerator;
  private eventGen: EventGenerator;
  private structuredGen: StructuredGenerator;

  constructor(config: Partial<SynthConfig> = {}) {
    // Validate and merge config
    const defaultConfig: SynthConfig = {
      provider: 'gemini',
      apiKey: process.env.GEMINI_API_KEY,
      model: 'gemini-2.0-flash-exp',
      cacheStrategy: 'memory',
      cacheTTL: 3600,
      maxRetries: 3,
      timeout: 30000,
      streaming: false,
      automation: false,
      vectorDB: false
    };

    this.config = SynthConfigSchema.parse({ ...defaultConfig, ...config });

    // Initialize generators
    this.timeSeriesGen = new TimeSeriesGenerator(this.config);
    this.eventGen = new EventGenerator(this.config);
    this.structuredGen = new StructuredGenerator(this.config);
  }

  /**
   * Generate time-series data
   */
  async generateTimeSeries<T = unknown>(
    options: Partial<TimeSeriesOptions> = {}
  ): Promise<GenerationResult<T>> {
    return this.timeSeriesGen.generate<T>(options as TimeSeriesOptions);
  }

  /**
   * Generate event data
   */
  async generateEvents<T = unknown>(
    options: Partial<EventOptions> = {}
  ): Promise<GenerationResult<T>> {
    return this.eventGen.generate<T>(options as EventOptions);
  }

  /**
   * Generate structured data
   */
  async generateStructured<T = unknown>(
    options: Partial<GeneratorOptions> = {}
  ): Promise<GenerationResult<T>> {
    return this.structuredGen.generate<T>(options as GeneratorOptions);
  }

  /**
   * Generate data by type
   */
  async generate<T = unknown>(
    type: DataType,
    options: Partial<GeneratorOptions> = {}
  ): Promise<GenerationResult<T>> {
    switch (type) {
      case 'timeseries':
        return this.generateTimeSeries<T>(options as TimeSeriesOptions);
      case 'events':
        return this.generateEvents<T>(options as EventOptions);
      case 'structured':
      case 'json':
        return this.generateStructured<T>(options);
      default:
        throw new Error(`Unsupported data type: ${type}`);
    }
  }

  /**
   * Generate with streaming
   */
  async *generateStream<T = unknown>(
    type: DataType,
    options: Partial<GeneratorOptions> = {}
  ): AsyncGenerator<T, void, unknown> {
    const generator = this.getGenerator(type);
    yield* generator.generateStream<T>(options as GeneratorOptions);
  }

  /**
   * Generate multiple batches in parallel
   */
  async generateBatch<T = unknown>(
    type: DataType,
    batchOptions: Partial<GeneratorOptions>[],
    concurrency: number = 3
  ): Promise<GenerationResult<T>[]> {
    const generator = this.getGenerator(type);
    return generator.generateBatch<T>(batchOptions as GeneratorOptions[], concurrency);
  }

  /**
   * Get generator for data type
   */
  private getGenerator(type: DataType) {
    switch (type) {
      case 'timeseries':
        return this.timeSeriesGen;
      case 'events':
        return this.eventGen;
      case 'structured':
      case 'json':
        return this.structuredGen;
      default:
        throw new Error(`Unsupported data type: ${type}`);
    }
  }

  /**
   * Configure instance
   */
  configure(config: Partial<SynthConfig>): void {
    this.config = SynthConfigSchema.parse({ ...this.config, ...config });

    // Recreate generators with new config
    this.timeSeriesGen = new TimeSeriesGenerator(this.config);
    this.eventGen = new EventGenerator(this.config);
    this.structuredGen = new StructuredGenerator(this.config);
  }

  /**
   * Get current configuration
   */
  getConfig(): SynthConfig {
    return { ...this.config };
  }
}

/**
 * Create a new AgenticSynth instance
 */
export function createSynth(config?: Partial<SynthConfig>): AgenticSynth {
  return new AgenticSynth(config);
}

// Export types and utilities
export * from './types.js';
export * from './generators/index.js';
export * from './cache/index.js';
export * from './routing/index.js';

// Default export
export default AgenticSynth;
