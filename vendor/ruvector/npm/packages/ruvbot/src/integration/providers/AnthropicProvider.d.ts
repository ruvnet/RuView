/**
 * AnthropicProvider - Anthropic Claude LLM Integration
 *
 * Provides Claude AI models (claude-3-opus, claude-3-sonnet, claude-3-haiku)
 * with streaming, tool use, and full API support.
 */
import type { LLMProvider, Message, CompletionOptions, StreamOptions, Completion, Token, ModelInfo } from './index.js';
export interface AnthropicConfig {
    apiKey: string;
    baseUrl?: string;
    model?: string;
    maxRetries?: number;
    timeout?: number;
}
export type AnthropicModel = 'claude-opus-4-20250514' | 'claude-sonnet-4-20250514' | 'claude-3-5-sonnet-20241022' | 'claude-3-5-haiku-20241022' | 'claude-3-opus-20240229' | 'claude-3-sonnet-20240229' | 'claude-3-haiku-20240307';
export declare class AnthropicProvider implements LLMProvider {
    private readonly config;
    private readonly model;
    constructor(config: AnthropicConfig);
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
    private convertMessages;
    private convertTools;
    private convertResponse;
    private makeRequest;
    private makeStreamRequest;
}
export declare function createAnthropicProvider(config: AnthropicConfig): AnthropicProvider;
export default AnthropicProvider;
//# sourceMappingURL=AnthropicProvider.d.ts.map