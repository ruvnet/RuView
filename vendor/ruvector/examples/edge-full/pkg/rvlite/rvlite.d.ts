/* tslint:disable */
/* eslint-disable */

export class BaseLoRA {
  free(): void;
  [Symbol.dispose](): void;
  distillFrom(micro: MicroLoRA, blend_factor: number): void;
  applyGradients(): void;
  constructor(config: LoraConfig);
  stats(): any;
  forward(input: Float32Array): Float32Array;
}

export class Embedder {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Get embedding dimensions
   */
  dimensions(): number;
  /**
   * Compute similarity between two texts
   */
  similarity(text_a: string, text_b: string): number;
  /**
   * Batch embed multiple texts
   * Takes JsValue array of strings, returns JsValue array of Float32Arrays
   */
  embed_batch(texts: any): any;
  /**
   * Create embedder with custom config
   */
  static with_config(config: EmbeddingConfig): Embedder;
  /**
   * Compute cosine similarity between two embeddings (JS arrays)
   */
  static cosine_similarity(a: Float32Array, b: Float32Array): number;
  /**
   * Create a new embedder with default config
   */
  constructor();
  /**
   * Generate embeddings for a single text
   * Uses hash-based projection for lightweight WASM operation
   * Returns Float32Array for direct JS consumption
   */
  embed(text: string): Float32Array;
}

export class EmbeddingConfig {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Create config for larger models
   */
  static with_dimensions(dimensions: number): EmbeddingConfig;
  constructor();
  /**
   * Create config for all-MiniLM-L6-v2 (default)
   */
  static minilm(): EmbeddingConfig;
  /**
   * Model dimensions (384 for all-MiniLM-L6-v2)
   */
  dimensions: number;
  /**
   * Normalize output vectors
   */
  normalize: boolean;
  /**
   * Max sequence length
   */
  max_length: number;
}

export class LoraConfig {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Create custom LoRA configuration
   */
  constructor(hidden_dim: number, rank: number, alpha: number, learning_rate: number);
  /**
   * Create BaseLoRA configuration (background training)
   */
  static base(hidden_dim: number): LoraConfig;
  /**
   * Create MicroLoRA configuration (per-request, <100μs)
   */
  static micro(hidden_dim: number): LoraConfig;
  /**
   * Export as JSON
   */
  toJSON(): any;
  /**
   * Validate configuration
   */
  validate(): void;
  /**
   * LoRA rank (1-2 for micro, 4-16 for base)
   */
  readonly rank: number;
  /**
   * Alpha scaling factor
   */
  readonly alpha: number;
  /**
   * Learning rate for adaptation
   */
  readonly learning_rate: number;
  /**
   * Hidden dimension
   */
  readonly hidden_dim: number;
}

export class MicroLoRA {
  free(): void;
  [Symbol.dispose](): void;
  exportWeights(): any;
  applyGradients(): void;
  accumulateGradient(input: Float32Array, feedback: number): void;
  constructor(config: LoraConfig);
  reset(): void;
  stats(): any;
  forward(input: Float32Array): Float32Array;
}

export class RvLite {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Get configuration
   */
  get_config(): any;
  /**
   * Get version string
   */
  get_version(): string;
  /**
   * Get enabled features
   */
  get_features(): any;
  /**
   * Insert a vector with a specific ID
   */
  insert_with_id(id: string, vector: Float32Array, metadata?: any | null): void;
  /**
   * Search with metadata filter
   */
  search_with_filter(query_vector: Float32Array, k: number, filter: any): any;
  /**
   * Get a vector by ID
   */
  get(id: string): any;
  /**
   * Get the number of vectors in the database
   */
  len(): number;
  /**
   * Create a new RvLite database
   */
  constructor(config: RvLiteConfig);
  /**
   * Execute SQL query (not yet implemented)
   */
  sql(_query: string): Promise<any>;
  /**
   * Execute Cypher query (not yet implemented)
   */
  cypher(_query: string): Promise<any>;
  /**
   * Delete a vector by ID
   */
  delete(id: string): boolean;
  /**
   * Insert a vector with optional metadata
   * Returns the vector ID
   */
  insert(vector: Float32Array, metadata?: any | null): string;
  /**
   * Search for similar vectors
   * Returns a JavaScript array of search results
   */
  search(query_vector: Float32Array, k: number): any;
  /**
   * Execute SPARQL query (not yet implemented)
   */
  sparql(_query: string): Promise<any>;
  /**
   * Create with default configuration (384 dimensions, cosine similarity)
   */
  static default(): RvLite;
  /**
   * Check if database is empty
   */
  is_empty(): boolean;
  /**
   * Check if database is ready
   */
  is_ready(): boolean;
}

export class RvLiteConfig {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Set distance metric (euclidean, cosine, dotproduct, manhattan)
   */
  with_distance_metric(metric: string): RvLiteConfig;
  constructor(dimensions: number);
}

export class TrmConfig {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Enable/disable attention variant
   */
  withAttention(use_attention: boolean): TrmConfig;
  /**
   * Set default K iterations
   */
  withDefaultK(k: number): TrmConfig;
  /**
   * Enable/disable early stopping
   */
  withEarlyStopping(enabled: boolean): TrmConfig;
  /**
   * Set latent iterations per K step
   */
  withLatentIterations(n: number): TrmConfig;
  /**
   * Set confidence threshold
   */
  withConfidenceThreshold(threshold: number): TrmConfig;
  /**
   * Create a new TRM configuration
   */
  constructor(embedding_dim: number, hidden_dim: number, max_k: number);
  /**
   * Create configuration optimized for speed
   */
  static fast(embedding_dim: number): TrmConfig;
  /**
   * Create configuration optimized for quality
   */
  static quality(embedding_dim: number): TrmConfig;
  /**
   * Export configuration as JSON
   */
  toJSON(): any;
  /**
   * Create balanced configuration (recommended)
   */
  static balanced(embedding_dim: number): TrmConfig;
  /**
   * Validate configuration
   */
  validate(): void;
  /**
   * Import configuration from JSON
   */
  static fromJSON(value: any): TrmConfig;
  /**
   * Embedding dimension (input/output size)
   */
  readonly embedding_dim: number;
  /**
   * Hidden dimension for latent state
   */
  readonly hidden_dim: number;
  /**
   * Maximum K iterations
   */
  readonly max_k: number;
  /**
   * Default K iterations
   */
  readonly default_k: number;
  /**
   * Latent updates per K iteration
   */
  readonly latent_iterations: number;
  /**
   * Use attention variant (more expressive, slower)
   */
  readonly use_attention: boolean;
  /**
   * Number of attention heads
   */
  readonly num_heads: number;
  /**
   * Confidence threshold for early stopping
   */
  readonly confidence_threshold: number;
  /**
   * Enable early stopping
   */
  readonly early_stopping: boolean;
  /**
   * Minimum iterations before early stopping
   */
  readonly min_iterations: number;
  /**
   * Convergence threshold for plateau detection
   */
  readonly convergence_threshold: number;
  /**
   * Residual scale for answer refinement
   */
  readonly residual_scale: number;
}

export class TrmEngine {
  free(): void;
  [Symbol.dispose](): void;
  getConfig(): any;
  reasonWithK(question: Float32Array, answer: Float32Array, k: number): TrmResult;
  constructor(config: TrmConfig);
  reset(): void;
  stats(): any;
  reason(question: Float32Array, answer: Float32Array): TrmResult;
}

export class TrmResult {
  private constructor();
  free(): void;
  [Symbol.dispose](): void;
  getAnswer(): Float32Array;
  toJSON(): any;
  readonly confidence: number;
  readonly iterations_used: number;
  readonly early_stopped: boolean;
  readonly latency_ms: number;
}

/**
 * Quick benchmark for embeddings
 */
export function benchmark_embeddings(iterations: number): any;

/**
 * Quick benchmark function
 */
export function benchmark_trm(iterations: number, hidden_dim: number): any;

/**
 * Compute cosine similarity
 */
export function cosineSimilarity(a: Float32Array, b: Float32Array): number;

/**
 * Dot product
 */
export function dotProduct(a: Float32Array, b: Float32Array): number;

/**
 * Get feature info
 */
export function features(): any;

export function init(): void;

/**
 * Compute L2 distance
 */
export function l2Distance(a: Float32Array, b: Float32Array): number;

/**
 * Linear interpolation
 */
export function lerp(a: Float32Array, b: Float32Array, t: number): Float32Array;

/**
 * Mean pooling for token embeddings
 */
export function meanPooling(embeddings: any, attention_mask?: Float32Array | null): Float32Array;

/**
 * Normalize a vector to unit length
 */
export function normalizeVector(vec: Float32Array): Float32Array;

/**
 * Create a random vector
 */
export function randomVector(dim: number, seed?: number | null): Float32Array;

/**
 * Softmax function
 */
export function softmax(vec: Float32Array): Float32Array;

/**
 * Get version string
 */
export function version(): string;

/**
 * Create a zero vector
 */
export function zeros(dim: number): Float32Array;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
  readonly memory: WebAssembly.Memory;
  readonly __wbg_baselora_free: (a: number, b: number) => void;
  readonly __wbg_embedder_free: (a: number, b: number) => void;
  readonly __wbg_embeddingconfig_free: (a: number, b: number) => void;
  readonly __wbg_get_embeddingconfig_dimensions: (a: number) => number;
  readonly __wbg_get_embeddingconfig_max_length: (a: number) => number;
  readonly __wbg_get_embeddingconfig_normalize: (a: number) => number;
  readonly __wbg_get_loraconfig_alpha: (a: number) => number;
  readonly __wbg_get_loraconfig_hidden_dim: (a: number) => number;
  readonly __wbg_get_loraconfig_learning_rate: (a: number) => number;
  readonly __wbg_get_trmconfig_confidence_threshold: (a: number) => number;
  readonly __wbg_get_trmconfig_convergence_threshold: (a: number) => number;
  readonly __wbg_get_trmconfig_early_stopping: (a: number) => number;
  readonly __wbg_get_trmconfig_latent_iterations: (a: number) => number;
  readonly __wbg_get_trmconfig_max_k: (a: number) => number;
  readonly __wbg_get_trmconfig_min_iterations: (a: number) => number;
  readonly __wbg_get_trmconfig_num_heads: (a: number) => number;
  readonly __wbg_get_trmconfig_residual_scale: (a: number) => number;
  readonly __wbg_get_trmconfig_use_attention: (a: number) => number;
  readonly __wbg_get_trmresult_confidence: (a: number) => number;
  readonly __wbg_get_trmresult_early_stopped: (a: number) => number;
  readonly __wbg_get_trmresult_iterations_used: (a: number) => number;
  readonly __wbg_get_trmresult_latency_ms: (a: number) => number;
  readonly __wbg_loraconfig_free: (a: number, b: number) => void;
  readonly __wbg_microlora_free: (a: number, b: number) => void;
  readonly __wbg_rvlite_free: (a: number, b: number) => void;
  readonly __wbg_rvliteconfig_free: (a: number, b: number) => void;
  readonly __wbg_set_embeddingconfig_dimensions: (a: number, b: number) => void;
  readonly __wbg_set_embeddingconfig_max_length: (a: number, b: number) => void;
  readonly __wbg_set_embeddingconfig_normalize: (a: number, b: number) => void;
  readonly __wbg_trmconfig_free: (a: number, b: number) => void;
  readonly __wbg_trmengine_free: (a: number, b: number) => void;
  readonly __wbg_trmresult_free: (a: number, b: number) => void;
  readonly baselora_applyGradients: (a: number) => void;
  readonly baselora_distillFrom: (a: number, b: number, c: number, d: number) => void;
  readonly baselora_forward: (a: number, b: number, c: number, d: number) => void;
  readonly baselora_new: (a: number, b: number) => void;
  readonly baselora_stats: (a: number) => number;
  readonly benchmark_embeddings: (a: number) => number;
  readonly benchmark_trm: (a: number, b: number) => number;
  readonly cosineSimilarity: (a: number, b: number, c: number, d: number, e: number) => void;
  readonly dotProduct: (a: number, b: number, c: number, d: number, e: number) => void;
  readonly embedder_cosine_similarity: (a: number, b: number) => number;
  readonly embedder_dimensions: (a: number) => number;
  readonly embedder_embed: (a: number, b: number, c: number) => number;
  readonly embedder_embed_batch: (a: number, b: number, c: number) => void;
  readonly embedder_new: () => number;
  readonly embedder_similarity: (a: number, b: number, c: number, d: number, e: number) => number;
  readonly embedder_with_config: (a: number) => number;
  readonly embeddingconfig_minilm: () => number;
  readonly embeddingconfig_with_dimensions: (a: number) => number;
  readonly features: () => number;
  readonly init: () => void;
  readonly l2Distance: (a: number, b: number, c: number, d: number, e: number) => void;
  readonly lerp: (a: number, b: number, c: number, d: number, e: number, f: number) => void;
  readonly loraconfig_base: (a: number) => number;
  readonly loraconfig_micro: (a: number) => number;
  readonly loraconfig_new: (a: number, b: number, c: number, d: number) => number;
  readonly loraconfig_toJSON: (a: number) => number;
  readonly loraconfig_validate: (a: number, b: number) => void;
  readonly meanPooling: (a: number, b: number, c: number, d: number) => void;
  readonly microlora_accumulateGradient: (a: number, b: number, c: number, d: number, e: number) => void;
  readonly microlora_applyGradients: (a: number) => void;
  readonly microlora_exportWeights: (a: number) => number;
  readonly microlora_forward: (a: number, b: number, c: number, d: number) => void;
  readonly microlora_new: (a: number, b: number) => void;
  readonly microlora_reset: (a: number) => void;
  readonly microlora_stats: (a: number) => number;
  readonly normalizeVector: (a: number, b: number, c: number) => void;
  readonly randomVector: (a: number, b: number, c: number) => void;
  readonly rvlite_cypher: (a: number, b: number, c: number) => number;
  readonly rvlite_default: (a: number) => void;
  readonly rvlite_delete: (a: number, b: number, c: number, d: number) => void;
  readonly rvlite_get: (a: number, b: number, c: number, d: number) => void;
  readonly rvlite_get_config: (a: number, b: number) => void;
  readonly rvlite_get_features: (a: number, b: number) => void;
  readonly rvlite_get_version: (a: number, b: number) => void;
  readonly rvlite_insert: (a: number, b: number, c: number, d: number, e: number) => void;
  readonly rvlite_insert_with_id: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => void;
  readonly rvlite_is_empty: (a: number, b: number) => void;
  readonly rvlite_is_ready: (a: number) => number;
  readonly rvlite_len: (a: number, b: number) => void;
  readonly rvlite_new: (a: number, b: number) => void;
  readonly rvlite_search: (a: number, b: number, c: number, d: number, e: number) => void;
  readonly rvlite_search_with_filter: (a: number, b: number, c: number, d: number, e: number, f: number) => void;
  readonly rvlite_sparql: (a: number, b: number, c: number) => number;
  readonly rvlite_sql: (a: number, b: number, c: number) => number;
  readonly rvliteconfig_new: (a: number) => number;
  readonly rvliteconfig_with_distance_metric: (a: number, b: number, c: number) => number;
  readonly softmax: (a: number, b: number, c: number) => void;
  readonly trmconfig_balanced: (a: number) => number;
  readonly trmconfig_fast: (a: number) => number;
  readonly trmconfig_fromJSON: (a: number, b: number) => void;
  readonly trmconfig_new: (a: number, b: number, c: number) => number;
  readonly trmconfig_quality: (a: number) => number;
  readonly trmconfig_toJSON: (a: number) => number;
  readonly trmconfig_validate: (a: number, b: number) => void;
  readonly trmconfig_withAttention: (a: number, b: number) => number;
  readonly trmconfig_withConfidenceThreshold: (a: number, b: number) => number;
  readonly trmconfig_withDefaultK: (a: number, b: number) => number;
  readonly trmconfig_withEarlyStopping: (a: number, b: number) => number;
  readonly trmconfig_withLatentIterations: (a: number, b: number) => number;
  readonly trmengine_getConfig: (a: number) => number;
  readonly trmengine_new: (a: number, b: number) => void;
  readonly trmengine_reason: (a: number, b: number, c: number, d: number, e: number, f: number) => void;
  readonly trmengine_reasonWithK: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => void;
  readonly trmengine_reset: (a: number) => void;
  readonly trmengine_stats: (a: number) => number;
  readonly trmresult_getAnswer: (a: number, b: number) => void;
  readonly trmresult_toJSON: (a: number) => number;
  readonly version: (a: number) => void;
  readonly zeros: (a: number, b: number) => void;
  readonly embeddingconfig_new: () => number;
  readonly __wbg_get_trmconfig_embedding_dim: (a: number) => number;
  readonly __wbg_get_loraconfig_rank: (a: number) => number;
  readonly __wbg_get_trmconfig_hidden_dim: (a: number) => number;
  readonly __wbg_get_trmconfig_default_k: (a: number) => number;
  readonly __wasm_bindgen_func_elem_690: (a: number, b: number, c: number) => void;
  readonly __wasm_bindgen_func_elem_684: (a: number, b: number) => void;
  readonly __wasm_bindgen_func_elem_1412: (a: number, b: number, c: number, d: number) => void;
  readonly __wbindgen_export: (a: number, b: number) => number;
  readonly __wbindgen_export2: (a: number, b: number, c: number, d: number) => number;
  readonly __wbindgen_export3: (a: number) => void;
  readonly __wbindgen_export4: (a: number, b: number, c: number) => void;
  readonly __wbindgen_add_to_stack_pointer: (a: number) => number;
  readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
* Instantiates the given `module`, which can either be bytes or
* a precompiled `WebAssembly.Module`.
*
* @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
*
* @returns {InitOutput}
*/
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
* If `module_or_path` is {RequestInfo} or {URL}, makes a request and
* for everything else, calls `WebAssembly.instantiate` directly.
*
* @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
*
* @returns {Promise<InitOutput>}
*/
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
