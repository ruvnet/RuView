/**
 * Streaming response support for RuvLLM
 */
import { StreamChunk, StreamOptions, QueryResponse, GenerationConfig } from './types';
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
export declare class StreamingGenerator {
    private llm;
    constructor(llm: {
        generate: (prompt: string, config?: GenerationConfig) => string;
        query: (text: string, config?: GenerationConfig) => QueryResponse;
    });
    /**
     * Stream response as async generator
     *
     * Note: This simulates streaming by chunking the full response.
     * Native streaming requires native module support.
     */
    stream(prompt: string, config?: GenerationConfig): AsyncGenerator<StreamChunk>;
    /**
     * Stream with callback handlers
     */
    streamWithCallbacks(prompt: string, options: StreamOptions): Promise<QueryResponse>;
    /**
     * Collect stream into single response
     */
    collect(prompt: string, config?: GenerationConfig): Promise<string>;
    private delay;
}
/**
 * Create a readable stream from response
 * (For Node.js stream compatibility)
 */
export declare function createReadableStream(generator: AsyncGenerator<StreamChunk>): ReadableStream<string>;
//# sourceMappingURL=streaming.d.ts.map