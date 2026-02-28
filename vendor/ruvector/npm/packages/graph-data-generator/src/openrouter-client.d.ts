/**
 * OpenRouter API client with Kimi K2 support
 */
import { OpenRouterConfig, OpenRouterRequest, OpenRouterResponse, OpenRouterMessage } from './types.js';
export declare class OpenRouterClient {
    private config;
    private throttledFetch;
    constructor(config?: Partial<OpenRouterConfig>);
    /**
     * Create a chat completion
     */
    createCompletion(messages: OpenRouterMessage[], options?: Partial<Omit<OpenRouterRequest, 'messages' | 'model'>>): Promise<OpenRouterResponse>;
    /**
     * Create a streaming chat completion
     */
    createStreamingCompletion(messages: OpenRouterMessage[], options?: Partial<Omit<OpenRouterRequest, 'messages' | 'model'>>): AsyncGenerator<string, void, unknown>;
    /**
     * Generate structured data using prompt engineering
     */
    generateStructured<T = unknown>(systemPrompt: string, userPrompt: string, options?: {
        temperature?: number;
        maxTokens?: number;
    }): Promise<T>;
    /**
     * Generate embeddings (if the model supports it)
     */
    generateEmbedding(_text: string): Promise<number[]>;
    /**
     * Update configuration
     */
    configure(config: Partial<OpenRouterConfig>): void;
    /**
     * Get current configuration
     */
    getConfig(): OpenRouterConfig;
}
/**
 * Create a new OpenRouter client
 */
export declare function createOpenRouterClient(config?: Partial<OpenRouterConfig>): OpenRouterClient;
//# sourceMappingURL=openrouter-client.d.ts.map