/**
 * Base generator class with API integration
 */
import { GoogleGenerativeAI } from '@google/generative-ai';
import { SynthConfig, GeneratorOptions, GenerationResult, StreamCallback } from '../types.js';
import { CacheManager } from '../cache/index.js';
import { ModelRouter } from '../routing/index.js';
export declare abstract class BaseGenerator<TOptions extends GeneratorOptions = GeneratorOptions> {
    protected config: SynthConfig;
    protected cache: CacheManager;
    protected router: ModelRouter;
    protected gemini?: GoogleGenerativeAI;
    constructor(config: SynthConfig);
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
    generate<T = unknown>(options: TOptions): Promise<GenerationResult<T>>;
    /**
     * Generate with streaming support
     */
    generateStream<T = unknown>(options: TOptions, callback?: StreamCallback<T>): AsyncGenerator<T, void, unknown>;
    /**
     * Batch generation with parallel processing
     */
    generateBatch<T = unknown>(batchOptions: TOptions[], concurrency?: number): Promise<GenerationResult<T>[]>;
    /**
     * Generate with specific model
     */
    private generateWithModel;
    /**
     * Call Gemini API
     */
    private callGemini;
    /**
     * Call OpenRouter API
     */
    private callOpenRouter;
    /**
     * Validate generation options
     */
    protected validateOptions(options: TOptions): void;
    /**
     * Try to parse items from streaming buffer
     */
    protected tryParseStreamBuffer(buffer: string, options: TOptions): unknown[];
    /**
     * Format output based on options
     */
    protected formatOutput(data: unknown[], format?: string): string | unknown[];
    /**
     * Convert data to CSV format
     */
    private convertToCSV;
}
//# sourceMappingURL=base.d.ts.map