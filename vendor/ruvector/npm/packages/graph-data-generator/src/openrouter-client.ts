/**
 * OpenRouter API client with Kimi K2 support
 */

import pRetry from 'p-retry';
import pThrottle from 'p-throttle';
import {
  OpenRouterConfig,
  OpenRouterConfigSchema,
  OpenRouterRequest,
  OpenRouterResponse,
  OpenRouterError,
  OpenRouterMessage
} from './types.js';

export class OpenRouterClient {
  private config: OpenRouterConfig;
  private throttledFetch: (url: string, options: RequestInit) => Promise<Response>;

  constructor(config: Partial<OpenRouterConfig> = {}) {
    const apiKey = config.apiKey || process.env.OPENROUTER_API_KEY;
    if (!apiKey) {
      throw new Error('OpenRouter API key is required. Set OPENROUTER_API_KEY environment variable or pass apiKey in config.');
    }

    this.config = OpenRouterConfigSchema.parse({ ...config, apiKey });

    // Setup rate limiting if configured
    if (this.config.rateLimit) {
      this.throttledFetch = pThrottle({
        limit: this.config.rateLimit.requests,
        interval: this.config.rateLimit.interval
      })(fetch.bind(globalThis)) as (url: string, options: RequestInit) => Promise<Response>;
    } else {
      this.throttledFetch = fetch.bind(globalThis);
    }
  }

  /**
   * Create a chat completion
   */
  async createCompletion(
    messages: OpenRouterMessage[],
    options: Partial<Omit<OpenRouterRequest, 'messages' | 'model'>> = {}
  ): Promise<OpenRouterResponse> {
    const request: OpenRouterRequest = {
      model: this.config.model || 'moonshot/kimi-k2-instruct',
      messages,
      temperature: options.temperature ?? 0.7,
      max_tokens: options.max_tokens ?? 4096,
      top_p: options.top_p ?? 1,
      stream: false,
      ...options
    };

    return pRetry(
      async () => {
        const response = await this.throttledFetch(
          `${this.config.baseURL}/chat/completions`,
          {
            method: 'POST',
            headers: {
              'Content-Type': 'application/json',
              'Authorization': `Bearer ${this.config.apiKey}`,
              'HTTP-Referer': 'https://github.com/ruvnet/ruvector',
              'X-Title': 'RuVector Graph Data Generator'
            },
            body: JSON.stringify(request),
            signal: AbortSignal.timeout(this.config.timeout || 60000)
          }
        );

        if (!response.ok) {
          const error = await response.text();
          throw new OpenRouterError(
            `OpenRouter API error: ${response.status} ${response.statusText}`,
            { status: response.status, error }
          );
        }

        const data = await response.json() as OpenRouterResponse;
        return data;
      },
      {
        retries: this.config.maxRetries || 3,
        onFailedAttempt: (error) => {
          console.warn(`Attempt ${error.attemptNumber} failed. ${error.retriesLeft} retries left.`);
        }
      }
    );
  }

  /**
   * Create a streaming chat completion
   */
  async *createStreamingCompletion(
    messages: OpenRouterMessage[],
    options: Partial<Omit<OpenRouterRequest, 'messages' | 'model'>> = {}
  ): AsyncGenerator<string, void, unknown> {
    const request: OpenRouterRequest = {
      model: this.config.model || 'moonshot/kimi-k2-instruct',
      messages,
      temperature: options.temperature ?? 0.7,
      max_tokens: options.max_tokens ?? 4096,
      top_p: options.top_p ?? 1,
      stream: true,
      ...options
    };

    const response = await this.throttledFetch(
      `${this.config.baseURL}/chat/completions`,
      {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          'Authorization': `Bearer ${this.config.apiKey}`,
          'HTTP-Referer': 'https://github.com/ruvnet/ruvector',
          'X-Title': 'RuVector Graph Data Generator'
        },
        body: JSON.stringify(request),
        signal: AbortSignal.timeout(this.config.timeout || 60000)
      }
    );

    if (!response.ok) {
      const error = await response.text();
      throw new OpenRouterError(
        `OpenRouter API error: ${response.status} ${response.statusText}`,
        { status: response.status, error }
      );
    }

    if (!response.body) {
      throw new OpenRouterError('No response body received');
    }

    const reader = response.body.getReader();
    const decoder = new TextDecoder();
    let buffer = '';

    try {
      while (true) {
        const { done, value } = await reader.read();
        if (done) break;

        buffer += decoder.decode(value, { stream: true });
        const lines = buffer.split('\n');
        buffer = lines.pop() || '';

        for (const line of lines) {
          if (line.startsWith('data: ')) {
            const data = line.slice(6).trim();
            if (data === '[DONE]') {
              return;
            }

            try {
              const parsed = JSON.parse(data);
              const content = parsed.choices?.[0]?.delta?.content;
              if (content) {
                yield content;
              }
            } catch (e) {
              console.warn('Failed to parse SSE data:', data);
            }
          }
        }
      }
    } finally {
      reader.releaseLock();
    }
  }

  /**
   * Generate structured data using prompt engineering
   */
  async generateStructured<T = unknown>(
    systemPrompt: string,
    userPrompt: string,
    options?: {
      temperature?: number;
      maxTokens?: number;
    }
  ): Promise<T> {
    const messages: OpenRouterMessage[] = [
      { role: 'system', content: systemPrompt },
      { role: 'user', content: userPrompt }
    ];

    const response = await this.createCompletion(messages, {
      temperature: options?.temperature ?? 0.7,
      max_tokens: options?.maxTokens ?? 4096
    });

    const content = response.choices[0]?.message?.content;
    if (!content) {
      throw new OpenRouterError('No content in response');
    }

    // Try to extract JSON from response
    try {
      // Look for JSON in code blocks
      const jsonMatch = content.match(/```(?:json)?\s*(\{[\s\S]*?\}|\[[\s\S]*?\])\s*```/);
      if (jsonMatch) {
        return JSON.parse(jsonMatch[1]) as T;
      }

      // Try to parse the entire response as JSON
      return JSON.parse(content) as T;
    } catch (e) {
      throw new OpenRouterError('Failed to parse JSON from response', { content, error: e });
    }
  }

  /**
   * Generate embeddings (if the model supports it)
   */
  async generateEmbedding(_text: string): Promise<number[]> {
    // Note: Kimi K2 may not support embeddings directly
    // This is a placeholder for potential future support
    throw new Error('Embedding generation not yet implemented for Kimi K2');
  }

  /**
   * Update configuration
   */
  configure(config: Partial<OpenRouterConfig>): void {
    this.config = OpenRouterConfigSchema.parse({ ...this.config, ...config });
  }

  /**
   * Get current configuration
   */
  getConfig(): OpenRouterConfig {
    return { ...this.config };
  }
}

/**
 * Create a new OpenRouter client
 */
export function createOpenRouterClient(config?: Partial<OpenRouterConfig>): OpenRouterClient {
  return new OpenRouterClient(config);
}
