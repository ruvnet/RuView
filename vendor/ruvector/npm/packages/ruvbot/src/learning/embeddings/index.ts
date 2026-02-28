/**
 * Embeddings Module - WASM-accelerated embedding generation
 */

export interface EmbeddingEngine {
  wasmRuntime: WasmEmbedder;
  batchProcessor: BatchEmbeddingProcessor;
  cache: EmbeddingCache;
}

export interface WasmEmbedder {
  initialize(): Promise<void>;
  embed(text: string): Promise<Float32Array>;
  embedBatch(texts: string[]): Promise<Float32Array[]>;
  dimensions(): number;
  dispose(): Promise<void>;
}

export interface BatchEmbeddingProcessor {
  queue(text: string): Promise<EmbeddingPromise>;
  flush(): Promise<Map<string, Float32Array>>;
  configure(options: BatchOptions): void;
}

export interface EmbeddingPromise {
  id: string;
  promise: Promise<Float32Array>;
}

export interface BatchOptions {
  maxBatchSize: number;
  maxWaitMs: number;
}

export interface EmbeddingCache {
  get(key: string): Promise<Float32Array | null>;
  set(key: string, embedding: Float32Array, ttl?: number): Promise<void>;
  delete(key: string): Promise<void>;
  clear(): Promise<void>;
}
