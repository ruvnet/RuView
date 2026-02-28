/**
 * GoogleAIProvider - Google AI (Gemini) LLM Integration
 *
 * Provides direct access to Google's Gemini models using the Google AI API.
 * Supports both API key authentication and Google Cloud default credentials.
 */
import type { LLMProvider, Message, CompletionOptions, StreamOptions, Completion, Token, ModelInfo } from './index.js';
export interface GoogleAIConfig {
    apiKey?: string;
    projectId?: string;
    location?: string;
    model?: string;
    maxRetries?: number;
    timeout?: number;
}
export type GoogleAIModel = 'gemini-3-pro-preview' | 'gemini-3-flash-preview' | 'gemini-2.5-pro' | 'gemini-2.5-flash' | 'gemini-2.5-flash-lite' | 'gemini-2.5-flash-image' | 'gemini-2.0-flash' | 'gemini-2.0-flash-lite' | 'gemini-1.5-pro' | 'gemini-1.5-flash' | 'gemini-1.5-flash-8b' | string;
export declare class GoogleAIProvider implements LLMProvider {
    private readonly config;
    private readonly baseUrl;
    private readonly modelId;
    constructor(config: GoogleAIConfig);
    complete(messages: Message[], options?: CompletionOptions): Promise<Completion>;
    stream(messages: Message[], options?: StreamOptions): AsyncGenerator<Token, Completion, void>;
    countTokens(text: string): Promise<number>;
    getModel(): ModelInfo;
    isHealthy(): Promise<boolean>;
    private convertMessages;
    private extractSystemInstruction;
    private mapFinishReason;
}
export declare function createGoogleAIProvider(config: GoogleAIConfig): LLMProvider;
//# sourceMappingURL=GoogleAIProvider.d.ts.map