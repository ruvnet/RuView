/**
 * Base generator class with API integration
 */

import { GoogleGenerativeAI } from '@google/generative-ai';
import {
  SynthConfig,
  GeneratorOptions,
  GenerationResult,
  ModelProvider,
  APIError,
  ValidationError,
  StreamCallback
} from '../types.js';
import { CacheManager } from '../cache/index.js';
import { ModelRouter } from '../routing/index.js';

export abstract class BaseGenerator<TOptions extends GeneratorOptions = GeneratorOptions> {
  protected config: SynthConfig;
  protected cache: CacheManager;
  protected router: ModelRouter;
  protected gemini?: GoogleGenerativeAI;

  constructor(config: SynthConfig) {
    this.config = config;

    // Initialize cache
    this.cache = new CacheManager({
      strategy: config.cacheStrategy || 'memory',
      ttl: config.cacheTTL || 3600,
      maxSize: 1000
    });

    // Initialize router with user configuration
    // Respect user's fallback preferences instead of hardcoding
    let fallbackChain: ModelProvider[] | undefined = undefined;

    // Only use fallback if explicitly enabled (default: true)
    if (config.enableFallback !== false) {
      if (config.fallbackChain && config.fallbackChain.length > 0) {
        // Use user-provided fallback chain
        fallbackChain = config.fallbackChain;
      } else {
        // Use default fallback chain
        // The router will still respect the user's primary provider choice
        // Fallback only triggers if primary provider fails
        fallbackChain = config.provider === 'gemini' ? ['openrouter'] : ['gemini'];
      }
    }

    this.router = new ModelRouter({
      defaultProvider: config.provider,
      providerKeys: {
        gemini: config.apiKey || process.env.GEMINI_API_KEY,
        openrouter: process.env.OPENROUTER_API_KEY
      },
      fallbackChain
    });

    // Initialize Gemini if needed
    const geminiKey = config.apiKey || process.env.GEMINI_API_KEY;
    if (config.provider === 'gemini' && geminiKey) {
      this.gemini = new GoogleGenerativeAI(geminiKey);
    }
  }

  /**
   * Abstract method for generation logic
   */
  protected abstract generatePrompt(options: TOptions): string;

  /**
   * Abstract method for result parsing
   */
  protected abstract parseResult(response: string, options: TOptions): unknown[];

  /**
   * Generate synthetic data
   */
  async generate<T = unknown>(options: TOptions): Promise<GenerationResult<T>> {
    const startTime = Date.now();

    // Validate options
    this.validateOptions(options);

    // Check cache
    const cacheKey = CacheManager.generateKey('generate', {
      type: this.constructor.name,
      options
    });

    const cached = await this.cache.get<GenerationResult<T>>(cacheKey);
    if (cached) {
      return {
        ...cached,
        metadata: {
          ...cached.metadata,
          cached: true
        }
      };
    }

    // Select model
    const route = this.router.selectModel({
      provider: this.config.provider,
      preferredModel: this.config.model,
      capabilities: ['text', 'json']
    });

    // Generate with retry logic
    let lastError: Error | null = null;
    const fallbackChain = this.router.getFallbackChain(route);

    for (const fallbackRoute of fallbackChain) {
      try {
        const result = await this.generateWithModel<T>(fallbackRoute, options, startTime);

        // Cache result
        await this.cache.set(cacheKey, result, this.config.cacheTTL);

        return result;
      } catch (error) {
        lastError = error as Error;
        console.warn(`Failed with ${fallbackRoute.model}, trying fallback...`);
      }
    }

    throw new APIError(
      `All model attempts failed: ${lastError?.message}`,
      { lastError, fallbackChain }
    );
  }

  /**
   * Generate with streaming support
   */
  async *generateStream<T = unknown>(
    options: TOptions,
    callback?: StreamCallback<T>
  ): AsyncGenerator<T, void, unknown> {
    if (!this.config.streaming) {
      throw new ValidationError('Streaming not enabled in configuration');
    }

    const prompt = this.generatePrompt(options);
    const route = this.router.selectModel({
      provider: this.config.provider,
      capabilities: ['streaming']
    });

    if (route.provider === 'gemini' && this.gemini) {
      const model = this.gemini.getGenerativeModel({ model: route.model });
      const result = await model.generateContentStream(prompt);

      let buffer = '';
      for await (const chunk of result.stream) {
        const text = chunk.text();
        buffer += text;

        // Try to parse complete items
        const items = this.tryParseStreamBuffer(buffer, options);
        for (const item of items) {
          if (callback) {
            await callback({ type: 'data', data: item as T });
          }
          yield item as T;
        }
      }
    } else {
      throw new APIError('Streaming not supported for this provider/model', {
        route
      });
    }

    if (callback) {
      await callback({ type: 'complete' });
    }
  }

  /**
   * Batch generation with parallel processing
   */
  async generateBatch<T = unknown>(
    batchOptions: TOptions[],
    concurrency: number = 3
  ): Promise<GenerationResult<T>[]> {
    const results: GenerationResult<T>[] = [];

    for (let i = 0; i < batchOptions.length; i += concurrency) {
      const batch = batchOptions.slice(i, i + concurrency);
      const batchResults = await Promise.all(
        batch.map(options => this.generate<T>(options))
      );
      results.push(...batchResults);
    }

    return results;
  }

  /**
   * Generate with specific model
   */
  private async generateWithModel<T>(
    route: ReturnType<ModelRouter['selectModel']>,
    options: TOptions,
    startTime: number
  ): Promise<GenerationResult<T>> {
    const prompt = this.generatePrompt(options);

    let response: string;
    if (route.provider === 'gemini' && this.gemini) {
      response = await this.callGemini(route.model, prompt);
    } else if (route.provider === 'openrouter') {
      response = await this.callOpenRouter(route.model, prompt);
    } else {
      throw new APIError(`Unsupported provider: ${route.provider}`, { route });
    }

    const data = this.parseResult(response, options) as T[];

    return {
      data,
      metadata: {
        count: data.length,
        generatedAt: new Date(),
        provider: route.provider,
        model: route.model,
        cached: false,
        duration: Date.now() - startTime
      }
    };
  }

  /**
   * Call Gemini API
   */
  private async callGemini(model: string, prompt: string): Promise<string> {
    if (!this.gemini) {
      throw new APIError('Gemini client not initialized', {
        provider: 'gemini'
      });
    }

    try {
      const genModel = this.gemini.getGenerativeModel({ model });
      const result = await genModel.generateContent(prompt);
      const response = result.response;
      return response.text();
    } catch (error) {
      const errorMessage = error instanceof Error ? error.message : 'Unknown error';
      throw new APIError(`Gemini API error: ${errorMessage}`, {
        model,
        error
      });
    }
  }

  /**
   * Call OpenRouter API
   */
  private async callOpenRouter(model: string, prompt: string): Promise<string> {
    const apiKey = process.env.OPENROUTER_API_KEY;
    if (!apiKey) {
      throw new APIError('OpenRouter API key not configured', {
        provider: 'openrouter'
      });
    }

    try {
      const response = await fetch('https://openrouter.ai/api/v1/chat/completions', {
        method: 'POST',
        headers: {
          'Authorization': `Bearer ${apiKey}`,
          'Content-Type': 'application/json'
        },
        body: JSON.stringify({
          model,
          messages: [{ role: 'user', content: prompt }]
        })
      });

      if (!response.ok) {
        throw new Error(`HTTP ${response.status}: ${response.statusText}`);
      }

      const data = await response.json() as {
        choices?: Array<{ message?: { content?: string } }>
      };
      return data.choices?.[0]?.message?.content || '';
    } catch (error) {
      const errorMessage = error instanceof Error ? error.message : 'Unknown error';
      throw new APIError(`OpenRouter API error: ${errorMessage}`, {
        model,
        error
      });
    }
  }

  /**
   * Validate generation options
   */
  protected validateOptions(options: TOptions): void {
    if (options.count !== undefined && options.count < 1) {
      throw new ValidationError('Count must be at least 1', { options });
    }

    if (options.format && !['json', 'csv', 'array'].includes(options.format)) {
      throw new ValidationError('Invalid format', { options });
    }
  }

  /**
   * Try to parse items from streaming buffer
   */
  protected tryParseStreamBuffer(buffer: string, options: TOptions): unknown[] {
    // Override in subclasses for specific parsing logic
    return [];
  }

  /**
   * Format output based on options
   */
  protected formatOutput(data: unknown[], format: string = 'json'): string | unknown[] {
    switch (format) {
      case 'csv':
        return this.convertToCSV(data);
      case 'array':
        return data;
      case 'json':
      default:
        return JSON.stringify(data, null, 2);
    }
  }

  /**
   * Convert data to CSV format
   */
  private convertToCSV(data: unknown[]): string {
    if (data.length === 0) return '';

    const firstItem = data[0];
    if (typeof firstItem !== 'object' || firstItem === null) return '';

    const headers = Object.keys(firstItem);
    const rows = data.map(item => {
      if (typeof item !== 'object' || item === null) return '';
      const record = item as Record<string, unknown>;
      return headers.map(header => JSON.stringify(record[header] ?? '')).join(',');
    });

    return [headers.join(','), ...rows].join('\n');
  }
}
