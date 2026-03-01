/* tslint:disable */
/* eslint-disable */

/**
 * Strategy for pooling token embeddings into a single sentence embedding
 */
export enum PoolingStrategy {
  /**
   * Average all token embeddings (most common)
   */
  Mean = 0,
  /**
   * Use only the [CLS] token embedding
   */
  Cls = 1,
  /**
   * Take the maximum value across all tokens for each dimension
   */
  Max = 2,
  /**
   * Mean pooling normalized by sqrt of sequence length
   */
  MeanSqrtLen = 3,
  /**
   * Use the last token embedding (for decoder models)
   */
  LastToken = 4,
}

export class WasmEmbedder {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Get maximum sequence length
   */
  maxLength(): number;
  /**
   * Compute similarity between two texts
   */
  similarity(text1: string, text2: string): number;
  /**
   * Generate embeddings for multiple texts
   */
  embedBatch(texts: string[]): Float32Array;
  /**
   * Create embedder with custom configuration
   */
  static withConfig(model_bytes: Uint8Array, tokenizer_json: string, config: WasmEmbedderConfig): WasmEmbedder;
  /**
   * Create a new embedder from model and tokenizer bytes
   *
   * # Arguments
   * * `model_bytes` - ONNX model file bytes
   * * `tokenizer_json` - Tokenizer JSON configuration
   */
  constructor(model_bytes: Uint8Array, tokenizer_json: string);
  /**
   * Get the embedding dimension
   */
  dimension(): number;
  /**
   * Generate embedding for a single text
   */
  embedOne(text: string): Float32Array;
}

export class WasmEmbedderConfig {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Set pooling strategy (0=Mean, 1=Cls, 2=Max, 3=MeanSqrtLen, 4=LastToken)
   */
  setPooling(pooling: number): WasmEmbedderConfig;
  /**
   * Set whether to normalize embeddings
   */
  setNormalize(normalize: boolean): WasmEmbedderConfig;
  /**
   * Set maximum sequence length
   */
  setMaxLength(max_length: number): WasmEmbedderConfig;
  /**
   * Create a new configuration
   */
  constructor();
}

/**
 * Compute cosine similarity between two embedding vectors (JS-friendly)
 */
export function cosineSimilarity(a: Float32Array, b: Float32Array): number;

/**
 * Initialize panic hook for better error messages in WASM
 */
export function init(): void;

/**
 * L2 normalize an embedding vector (JS-friendly)
 */
export function normalizeL2(embedding: Float32Array): Float32Array;

/**
 * Check if SIMD is available (for performance info)
 * Returns true if compiled with WASM SIMD128 support
 */
export function simd_available(): boolean;

/**
 * Get the library version
 */
export function version(): string;
