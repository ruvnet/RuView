/**
 * Stream processing utilities for lean agentic learning
 */

import { EventEmitter } from 'events';
import { ProcessingResult, Context } from './types';
import { LeanAgenticClient } from './client';

export interface StreamChunk {
  content: string;
  timestamp: number;
  metadata?: Record<string, any>;
}

export class StreamProcessor extends EventEmitter {
  private client: LeanAgenticClient;
  private context: Context;
  private chunkBuffer: StreamChunk[] = [];

  constructor(client: LeanAgenticClient, sessionId: string) {
    super();
    this.client = client;
    this.context = client.createContext(sessionId);
  }

  /**
   * Process a stream chunk
   */
  async processChunk(chunk: StreamChunk): Promise<ProcessingResult> {
    this.chunkBuffer.push(chunk);

    // Update context
    this.context.history.push(chunk.content);
    this.context.timestamp = chunk.timestamp;

    // Process through lean agentic system
    const result = await this.client.processChunk(
      chunk.content,
      this.context
    );

    // Emit events
    this.emit('chunk_processed', { chunk, result });

    if (result.reward > 0.8) {
      this.emit('high_reward', result);
    }

    return result;
  }

  /**
   * Process multiple chunks
   */
  async processStream(chunks: StreamChunk[]): Promise<ProcessingResult[]> {
    const results: ProcessingResult[] = [];

    for (const chunk of chunks) {
      const result = await this.processChunk(chunk);
      results.push(result);
    }

    this.emit('stream_complete', { results });

    return results;
  }

  /**
   * Get current context
   */
  getContext(): Context {
    return { ...this.context };
  }

  /**
   * Update context preferences
   */
  updatePreference(key: string, value: number): void {
    this.context.preferences[key] = value;
  }

  /**
   * Clear buffer
   */
  clearBuffer(): void {
    this.chunkBuffer = [];
  }

  /**
   * Get buffer size
   */
  getBufferSize(): number {
    return this.chunkBuffer.length;
  }
}

/**
 * Create a stream from an async iterator
 */
export async function* streamFromAsyncIterator<T>(
  iterator: AsyncIterableIterator<T>
): AsyncGenerator<StreamChunk> {
  for await (const item of iterator) {
    yield {
      content: String(item),
      timestamp: Date.now(),
    };
  }
}

/**
 * Create a batched stream processor
 */
export class BatchedStreamProcessor extends StreamProcessor {
  private batchSize: number;
  private currentBatch: StreamChunk[] = [];

  constructor(client: LeanAgenticClient, sessionId: string, batchSize: number = 10) {
    super(client, sessionId);
    this.batchSize = batchSize;
  }

  async processChunk(chunk: StreamChunk): Promise<ProcessingResult> {
    this.currentBatch.push(chunk);

    if (this.currentBatch.length >= this.batchSize) {
      return this.processBatch();
    }

    // Return a pending result
    return {
      action: {
        actionType: 'buffer',
        description: 'Buffering chunk',
        parameters: {},
        toolCalls: [],
        expectedReward: 0,
      },
      observation: {
        success: true,
        result: 'Buffered',
        changes: [],
        timestamp: Date.now(),
      },
      reward: 0,
      verified: false,
    };
  }

  private async processBatch(): Promise<ProcessingResult> {
    const combined = this.currentBatch.map(c => c.content).join(' ');
    const result = await super.processChunk({
      content: combined,
      timestamp: Date.now(),
    });

    this.currentBatch = [];
    this.emit('batch_processed', { result });

    return result;
  }
}
