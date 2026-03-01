/**
 * AI Provider Integration - LLM and Embedding Providers
 */

export interface ProviderRegistry {
  llm: LLMProvider;
  embedding: EmbeddingProvider;
}

export interface LLMProvider {
  complete(messages: Message[], options?: CompletionOptions): Promise<Completion>;
  stream(messages: Message[], options?: StreamOptions): AsyncGenerator<Token, Completion, void>;
  countTokens(text: string): Promise<number>;
  getModel(): ModelInfo;
  isHealthy(): Promise<boolean>;
}

export interface Message {
  role: 'user' | 'assistant' | 'system';
  content: string;
}

export interface CompletionOptions {
  maxTokens?: number;
  temperature?: number;
  topP?: number;
  stopSequences?: string[];
  tools?: Tool[];
}

export interface StreamOptions extends CompletionOptions {
  onToken?: (token: string) => void;
}

export interface Completion {
  content: string;
  finishReason: 'stop' | 'length' | 'tool_use';
  usage: {
    inputTokens: number;
    outputTokens: number;
  };
  toolCalls?: ToolCall[];
}

export interface Token {
  type: 'text' | 'tool_use';
  text?: string;
  toolUse?: ToolCall;
}

export interface Tool {
  name: string;
  description: string;
  parameters: Record<string, unknown>;
}

export interface ToolCall {
  id: string;
  name: string;
  input: Record<string, unknown>;
}

export interface ModelInfo {
  id: string;
  name: string;
  maxTokens: number;
  contextWindow: number;
}

export interface EmbeddingProvider {
  embed(texts: string[]): Promise<Float32Array[]>;
  embedSingle(text: string): Promise<Float32Array>;
  dimensions(): number;
  model(): string;
}

// Provider implementations
export {
  AnthropicProvider,
  createAnthropicProvider,
  type AnthropicConfig,
  type AnthropicModel,
} from './AnthropicProvider.js';

export {
  OpenRouterProvider,
  createOpenRouterProvider,
  createQwQProvider,
  createDeepSeekR1Provider,
  type OpenRouterConfig,
  type OpenRouterModel,
} from './OpenRouterProvider.js';

export {
  createGoogleAIProvider,
  type GoogleAIConfig,
  type GoogleAIModel,
} from './GoogleAIProvider.js';
