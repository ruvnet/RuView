/**
 * OpenRouterProvider - OpenRouter Multi-Model LLM Integration
 *
 * Provides access to multiple LLM providers through OpenRouter,
 * including Qwen QwQ reasoning models, Claude, GPT-4, and more.
 */
import type { LLMProvider, Message, CompletionOptions, StreamOptions, Completion, Token, ModelInfo } from './index.js';
export interface OpenRouterConfig {
    apiKey: string;
    baseUrl?: string;
    model?: string;
    siteUrl?: string;
    siteName?: string;
    maxRetries?: number;
    timeout?: number;
}
export type OpenRouterModel = 'qwen/qwq-32b' | 'qwen/qwq-32b:free' | 'qwen/qwq-32b-preview' | 'qwen/qwen3-max' | 'qwen/qwen-2.5-72b-instruct' | 'qwen/qwen-2.5-coder-32b-instruct' | 'anthropic/claude-3.5-sonnet' | 'anthropic/claude-3-opus' | 'anthropic/claude-3-haiku' | 'openai/gpt-4-turbo' | 'openai/gpt-4o' | 'openai/o1-preview' | 'openai/o1-mini' | 'google/gemini-2.5-pro-preview-05-06' | 'google/gemini-2.0-flash-001' | 'google/gemini-2.0-flash-lite-001' | 'google/gemini-2.0-flash-thinking-exp:free' | 'google/gemini-pro-1.5' | 'google/gemini-flash-1.5' | 'meta-llama/llama-3.1-405b-instruct' | 'meta-llama/llama-3.1-70b-instruct' | 'deepseek/deepseek-r1' | 'deepseek/deepseek-chat' | string;
export declare class OpenRouterProvider implements LLMProvider {
    private readonly config;
    private readonly model;
    constructor(config: OpenRouterConfig);
    /**
     * Complete a conversation
     */
    complete(messages: Message[], options?: CompletionOptions): Promise<Completion>;
    /**
     * Stream a conversation
     */
    stream(messages: Message[], options?: StreamOptions): AsyncGenerator<Token, Completion, void>;
    /**
     * Count tokens in text
     */
    countTokens(text: string): Promise<number>;
    /**
     * Get model info
     */
    getModel(): ModelInfo;
    /**
     * Check provider health
     */
    isHealthy(): Promise<boolean>;
    /**
     * List available models
     */
    listModels(): Promise<string[]>;
    private convertMessages;
    private convertTools;
    private convertResponse;
    private makeRequest;
    private makeStreamRequest;
}
export declare function createOpenRouterProvider(config: OpenRouterConfig): OpenRouterProvider;
/**
 * Create a provider for Qwen QwQ reasoning model
 */
export declare function createQwQProvider(apiKey: string, free?: boolean): OpenRouterProvider;
/**
 * Create a provider for DeepSeek R1 reasoning model
 */
export declare function createDeepSeekR1Provider(apiKey: string): OpenRouterProvider;
export default OpenRouterProvider;
//# sourceMappingURL=OpenRouterProvider.d.ts.map