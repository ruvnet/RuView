/**
 * Streaming response support for RuvLLM
 */

import {
  StreamChunk,
  StreamOptions,
  QueryResponse,
  GenerationConfig,
} from './types';

/**
 * Async generator for streaming responses
 *
 * @example
 * ```typescript
 * import { RuvLLM, StreamingGenerator } from '@ruvector/ruvllm';
 *
 * const llm = new RuvLLM();
 * const streamer = new StreamingGenerator(llm);
 *
 * // Stream with async iterator
 * for await (const chunk of streamer.stream('Write a story')) {
 *   process.stdout.write(chunk.text);
 * }
 *
 * // Stream with callbacks
 * await streamer.streamWithCallbacks('Write a poem', {
 *   onChunk: (chunk) => console.log(chunk.text),
 *   onComplete: (response) => console.log('Done!', response.latencyMs),
 * });
 * ```
 */
export class StreamingGenerator {
  private llm: {
    generate: (prompt: string, config?: GenerationConfig) => string;
    query: (text: string, config?: GenerationConfig) => QueryResponse;
  };

  constructor(llm: {
    generate: (prompt: string, config?: GenerationConfig) => string;
    query: (text: string, config?: GenerationConfig) => QueryResponse;
  }) {
    this.llm = llm;
  }

  /**
   * Stream response as async generator
   *
   * Note: This simulates streaming by chunking the full response.
   * Native streaming requires native module support.
   */
  async *stream(
    prompt: string,
    config?: GenerationConfig
  ): AsyncGenerator<StreamChunk> {
    const start = Date.now();

    // Generate full response (native streaming would yield real chunks)
    const fullText = this.llm.generate(prompt, config);

    // Simulate streaming by yielding words
    const words = fullText.split(/(\s+)/);
    let accumulated = '';
    let tokenCount = 0;

    for (let i = 0; i < words.length; i++) {
      accumulated += words[i];
      tokenCount++;

      // Yield every few tokens or at end
      if (tokenCount % 3 === 0 || i === words.length - 1) {
        yield {
          text: words.slice(Math.max(0, i - 2), i + 1).join(''),
          done: i === words.length - 1,
          tokenCount,
          latencyMs: Date.now() - start,
        };

        // Small delay to simulate streaming
        await this.delay(10);
      }
    }
  }

  /**
   * Stream with callback handlers
   */
  async streamWithCallbacks(
    prompt: string,
    options: StreamOptions
  ): Promise<QueryResponse> {
    const start = Date.now();
    let fullText = '';
    let tokenCount = 0;

    try {
      for await (const chunk of this.stream(prompt, options)) {
        fullText += chunk.text;
        tokenCount = chunk.tokenCount;

        if (options.onChunk) {
          options.onChunk(chunk);
        }
      }

      const response: QueryResponse = {
        text: fullText.trim(),
        confidence: 0.8,
        model: 'streaming',
        contextSize: tokenCount,
        latencyMs: Date.now() - start,
        requestId: `stream-${Date.now()}-${Math.random().toString(36).slice(2)}`,
      };

      if (options.onComplete) {
        options.onComplete(response);
      }

      return response;
    } catch (error) {
      if (options.onError) {
        options.onError(error as Error);
      }
      throw error;
    }
  }

  /**
   * Collect stream into single response
   */
  async collect(prompt: string, config?: GenerationConfig): Promise<string> {
    let result = '';
    for await (const chunk of this.stream(prompt, config)) {
      result = chunk.text; // Each chunk is cumulative
    }
    return result.trim();
  }

  private delay(ms: number): Promise<void> {
    return new Promise(resolve => setTimeout(resolve, ms));
  }
}

/**
 * Create a readable stream from response
 * (For Node.js stream compatibility)
 */
export function createReadableStream(
  generator: AsyncGenerator<StreamChunk>
): ReadableStream<string> {
  return new ReadableStream({
    async pull(controller) {
      const { value, done } = await generator.next();
      if (done) {
        controller.close();
      } else {
        controller.enqueue(value.text);
      }
    },
  });
}
