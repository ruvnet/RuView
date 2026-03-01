/**
 * WasmEmbedder - WASM-based Text Embedding
 *
 * Provides high-performance text embeddings using RuVector WASM bindings.
 * Supports batching, caching, and SIMD optimization.
 */

import type { Embedder } from '../memory/MemoryManager.js';
import { WasmError } from '../../core/errors.js';

// ============================================================================
// Types
// ============================================================================

export interface WasmEmbedderConfig {
  dimensions: number;
  modelPath?: string;
  cacheSize?: number;
  useSIMD?: boolean;
  batchSize?: number;
}

export interface EmbeddingCache {
  get(key: string): Float32Array | undefined;
  set(key: string, value: Float32Array): void;
  clear(): void;
  size(): number;
}

// ============================================================================
// Simple LRU Cache Implementation
// ============================================================================

class LRUCache implements EmbeddingCache {
  private cache: Map<string, Float32Array> = new Map();
  private readonly maxSize: number;

  constructor(maxSize: number = 10000) {
    this.maxSize = maxSize;
  }

  get(key: string): Float32Array | undefined {
    const value = this.cache.get(key);
    if (value) {
      // Move to end (most recently used)
      this.cache.delete(key);
      this.cache.set(key, value);
    }
    return value;
  }

  set(key: string, value: Float32Array): void {
    if (this.cache.has(key)) {
      this.cache.delete(key);
    } else if (this.cache.size >= this.maxSize) {
      // Remove oldest entry
      const firstKey = this.cache.keys().next().value;
      if (firstKey) {
        this.cache.delete(firstKey);
      }
    }
    this.cache.set(key, value);
  }

  clear(): void {
    this.cache.clear();
  }

  size(): number {
    return this.cache.size;
  }
}

// ============================================================================
// WasmEmbedder Implementation
// ============================================================================

export class WasmEmbedder implements Embedder {
  private readonly config: WasmEmbedderConfig;
  private readonly cache: EmbeddingCache;
  private wasmModule: unknown = null;
  private initialized: boolean = false;

  constructor(config: WasmEmbedderConfig) {
    this.config = {
      dimensions: config.dimensions,
      modelPath: config.modelPath,
      cacheSize: config.cacheSize ?? 10000,
      useSIMD: config.useSIMD ?? true,
      batchSize: config.batchSize ?? 32,
    };
    this.cache = new LRUCache(this.config.cacheSize);
  }

  /**
   * Initialize the WASM module
   */
  async initialize(): Promise<void> {
    if (this.initialized) return;

    try {
      // Try to load @ruvector/ruvllm (WASM module)
      try {
        // Dynamic import - may not be available
        const ruvllm = await import('@ruvector/ruvllm');
        this.wasmModule = ruvllm;
      } catch {
        // Use fallback embedder if no WASM available
        console.warn('No WASM module available, using fallback embedder');
      }

      this.initialized = true;
    } catch (error) {
      throw new WasmError(
        `Failed to initialize WASM embedder: ${error instanceof Error ? error.message : 'Unknown error'}`,
        { config: this.config }
      );
    }
  }

  /**
   * Embed a single text string
   */
  async embed(text: string): Promise<Float32Array> {
    if (!this.initialized) {
      await this.initialize();
    }

    // Check cache
    const cached = this.cache.get(text);
    if (cached) {
      return cached;
    }

    // Generate embedding
    const embedding = await this.generateEmbedding(text);

    // Cache result
    this.cache.set(text, embedding);

    return embedding;
  }

  /**
   * Embed multiple texts in batch
   */
  async embedBatch(texts: string[]): Promise<Float32Array[]> {
    if (!this.initialized) {
      await this.initialize();
    }

    const results: Float32Array[] = [];
    const uncached: { index: number; text: string }[] = [];

    // Check cache for each text
    for (let i = 0; i < texts.length; i++) {
      const cached = this.cache.get(texts[i]);
      if (cached) {
        results[i] = cached;
      } else {
        uncached.push({ index: i, text: texts[i] });
      }
    }

    // Generate embeddings for uncached texts in batches
    const batchSize = this.config.batchSize!;
    for (let i = 0; i < uncached.length; i += batchSize) {
      const batch = uncached.slice(i, i + batchSize);
      const batchTexts = batch.map(item => item.text);

      const embeddings = await this.generateEmbeddingBatch(batchTexts);

      for (let j = 0; j < batch.length; j++) {
        const embedding = embeddings[j];
        results[batch[j].index] = embedding;
        this.cache.set(batch[j].text, embedding);
      }
    }

    return results;
  }

  /**
   * Get embedding dimensions
   */
  dimension(): number {
    return this.config.dimensions;
  }

  /**
   * Clear the embedding cache
   */
  clearCache(): void {
    this.cache.clear();
  }

  /**
   * Get cache statistics
   */
  getCacheStats(): { size: number; maxSize: number } {
    return {
      size: this.cache.size(),
      maxSize: this.config.cacheSize!,
    };
  }

  // ==========================================================================
  // Private Methods
  // ==========================================================================

  private async generateEmbedding(text: string): Promise<Float32Array> {
    if (this.wasmModule) {
      // Use WASM module if available
      const module = this.wasmModule as {
        embed?: (text: string) => Float32Array;
        RuvLLM?: { embed: (text: string) => Promise<Float32Array> };
      };

      if (module.embed) {
        return module.embed(text);
      }
      if (module.RuvLLM) {
        return module.RuvLLM.embed(text);
      }
    }

    // Fallback: Generate deterministic pseudo-random embedding
    return this.fallbackEmbed(text);
  }

  private async generateEmbeddingBatch(texts: string[]): Promise<Float32Array[]> {
    if (this.wasmModule) {
      const module = this.wasmModule as {
        embedBatch?: (texts: string[]) => Float32Array[];
      };

      if (module.embedBatch) {
        return module.embedBatch(texts);
      }
    }

    // Fallback: Generate individually
    return Promise.all(texts.map(text => this.generateEmbedding(text)));
  }

  private fallbackEmbed(text: string): Float32Array {
    // Generate deterministic embedding based on text hash
    // This is for testing/development when WASM is not available
    const embedding = new Float32Array(this.config.dimensions);
    let hash = this.hashCode(text);

    for (let i = 0; i < this.config.dimensions; i++) {
      hash = ((hash * 1103515245) + 12345) & 0x7fffffff;
      embedding[i] = (hash / 0x7fffffff) * 2 - 1;
    }

    // Normalize
    const norm = Math.sqrt(embedding.reduce((sum, val) => sum + val * val, 0));
    for (let i = 0; i < this.config.dimensions; i++) {
      embedding[i] /= norm;
    }

    return embedding;
  }

  private hashCode(str: string): number {
    let hash = 0;
    for (let i = 0; i < str.length; i++) {
      const char = str.charCodeAt(i);
      hash = ((hash << 5) - hash) + char;
      hash = hash & hash;
    }
    return Math.abs(hash);
  }
}

// ============================================================================
// Factory Function
// ============================================================================

export function createWasmEmbedder(config: WasmEmbedderConfig): WasmEmbedder {
  return new WasmEmbedder(config);
}

export default WasmEmbedder;
