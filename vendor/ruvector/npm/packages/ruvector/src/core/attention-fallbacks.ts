/**
 * Attention Fallbacks - Safe wrapper around @ruvector/attention with automatic array conversion
 *
 * This wrapper handles the array type conversion automatically, allowing users
 * to pass either regular arrays or Float32Arrays.
 *
 * @ruvector/attention requires Float32Array inputs.
 * This wrapper handles the conversion automatically.
 */

// Lazy load to avoid import errors if not installed
let attentionModule: any = null;
let loadError: Error | null = null;

function getAttentionModule() {
  if (attentionModule) return attentionModule;
  if (loadError) throw loadError;

  try {
    attentionModule = require('@ruvector/attention');
    return attentionModule;
  } catch (e: any) {
    loadError = new Error(
      `@ruvector/attention is not installed or failed to load: ${e.message}\n` +
      `Install with: npm install @ruvector/attention`
    );
    throw loadError;
  }
}

/**
 * Convert any array-like input to Float32Array
 */
function toFloat32Array(input: number[] | Float32Array | Float64Array): Float32Array {
  if (input instanceof Float32Array) {
    return input;
  }
  return new Float32Array(input);
}

/**
 * Convert nested arrays to Float32Arrays
 */
function toFloat32Arrays(inputs: (number[] | Float32Array | Float64Array)[]): Float32Array[] {
  return inputs.map(arr => toFloat32Array(arr));
}

/**
 * Convert Float32Array result back to regular array if needed
 */
function fromFloat32Array(input: Float32Array): number[] {
  return Array.from(input);
}

/**
 * Attention output interface
 */
export interface AttentionOutput {
  /** Output vector as regular array */
  values: number[];
  /** Output as Float32Array for performance-critical code */
  raw: Float32Array;
}

/**
 * Multi-head attention mechanism
 *
 * This wrapper automatically converts array inputs to Float32Array.
 */
export class MultiHeadAttention {
  private inner: any;
  public readonly dim: number;
  public readonly numHeads: number;

  /**
   * Create a new multi-head attention instance
   *
   * @param dim - Embedding dimension (must be divisible by numHeads)
   * @param numHeads - Number of attention heads
   */
  constructor(dim: number, numHeads: number) {
    const attention = getAttentionModule();
    this.inner = new attention.MultiHeadAttention(dim, numHeads);
    this.dim = dim;
    this.numHeads = numHeads;
  }

  /**
   * Compute multi-head attention
   *
   * @param query - Query vector
   * @param keys - Array of key vectors
   * @param values - Array of value vectors
   * @returns Attention output
   *
   * @example
   * ```typescript
   * const mha = new MultiHeadAttention(64, 4);
   *
   * // Works with regular arrays
   * const result1 = mha.compute([...64 values], [[...64], [...64]], [[...64], [...64]]);
   *
   * // Also works with Float32Array
   * const q = new Float32Array(64);
   * const k = [new Float32Array(64)];
   * const v = [new Float32Array(64)];
   * const result2 = mha.compute(q, k, v);
   * ```
   */
  compute(
    query: number[] | Float32Array,
    keys: (number[] | Float32Array)[],
    values: (number[] | Float32Array)[]
  ): AttentionOutput {
    const raw = this.inner.compute(
      toFloat32Array(query),
      toFloat32Arrays(keys),
      toFloat32Arrays(values)
    );
    return {
      values: fromFloat32Array(raw),
      raw
    };
  }

  /**
   * Compute and return raw Float32Array (faster, no conversion)
   */
  computeRaw(
    query: Float32Array,
    keys: Float32Array[],
    values: Float32Array[]
  ): Float32Array {
    return this.inner.compute(query, keys, values);
  }

  get headDim(): number {
    return this.dim / this.numHeads;
  }
}

/**
 * Flash attention with tiled computation
 */
export class FlashAttention {
  private inner: any;
  public readonly dim: number;
  public readonly blockSize: number;

  /**
   * Create a new flash attention instance
   *
   * @param dim - Embedding dimension
   * @param blockSize - Block size for tiled computation (default: 512)
   */
  constructor(dim: number, blockSize: number = 512) {
    const attention = getAttentionModule();
    this.inner = new attention.FlashAttention(dim, blockSize);
    this.dim = dim;
    this.blockSize = blockSize;
  }

  /**
   * Compute flash attention
   */
  compute(
    query: number[] | Float32Array,
    keys: (number[] | Float32Array)[],
    values: (number[] | Float32Array)[]
  ): AttentionOutput {
    const raw = this.inner.compute(
      toFloat32Array(query),
      toFloat32Arrays(keys),
      toFloat32Arrays(values)
    );
    return {
      values: fromFloat32Array(raw),
      raw
    };
  }

  computeRaw(
    query: Float32Array,
    keys: Float32Array[],
    values: Float32Array[]
  ): Float32Array {
    return this.inner.compute(query, keys, values);
  }
}

/**
 * Hyperbolic attention in Poincare ball model
 */
export class HyperbolicAttention {
  private inner: any;
  public readonly dim: number;
  public readonly curvature: number;

  /**
   * Create a new hyperbolic attention instance
   *
   * @param dim - Embedding dimension
   * @param curvature - Hyperbolic curvature (typically 1.0)
   */
  constructor(dim: number, curvature: number = 1.0) {
    const attention = getAttentionModule();
    this.inner = new attention.HyperbolicAttention(dim, curvature);
    this.dim = dim;
    this.curvature = curvature;
  }

  /**
   * Compute hyperbolic attention
   */
  compute(
    query: number[] | Float32Array,
    keys: (number[] | Float32Array)[],
    values: (number[] | Float32Array)[]
  ): AttentionOutput {
    const raw = this.inner.compute(
      toFloat32Array(query),
      toFloat32Arrays(keys),
      toFloat32Arrays(values)
    );
    return {
      values: fromFloat32Array(raw),
      raw
    };
  }

  computeRaw(
    query: Float32Array,
    keys: Float32Array[],
    values: Float32Array[]
  ): Float32Array {
    return this.inner.compute(query, keys, values);
  }
}

/**
 * Linear attention (Performer-style) with O(n) complexity
 */
export class LinearAttention {
  private inner: any;
  public readonly dim: number;
  public readonly numFeatures: number;

  /**
   * Create a new linear attention instance
   *
   * @param dim - Embedding dimension
   * @param numFeatures - Number of random features
   */
  constructor(dim: number, numFeatures: number) {
    const attention = getAttentionModule();
    this.inner = new attention.LinearAttention(dim, numFeatures);
    this.dim = dim;
    this.numFeatures = numFeatures;
  }

  /**
   * Compute linear attention
   */
  compute(
    query: number[] | Float32Array,
    keys: (number[] | Float32Array)[],
    values: (number[] | Float32Array)[]
  ): AttentionOutput {
    const raw = this.inner.compute(
      toFloat32Array(query),
      toFloat32Arrays(keys),
      toFloat32Arrays(values)
    );
    return {
      values: fromFloat32Array(raw),
      raw
    };
  }

  computeRaw(
    query: Float32Array,
    keys: Float32Array[],
    values: Float32Array[]
  ): Float32Array {
    return this.inner.compute(query, keys, values);
  }
}

/**
 * Local-global attention (Longformer-style)
 */
export class LocalGlobalAttention {
  private inner: any;
  public readonly dim: number;
  public readonly localWindow: number;
  public readonly globalTokens: number;

  /**
   * Create a new local-global attention instance
   *
   * @param dim - Embedding dimension
   * @param localWindow - Size of local attention window
   * @param globalTokens - Number of global attention tokens
   */
  constructor(dim: number, localWindow: number, globalTokens: number) {
    const attention = getAttentionModule();
    this.inner = new attention.LocalGlobalAttention(dim, localWindow, globalTokens);
    this.dim = dim;
    this.localWindow = localWindow;
    this.globalTokens = globalTokens;
  }

  /**
   * Compute local-global attention
   */
  compute(
    query: number[] | Float32Array,
    keys: (number[] | Float32Array)[],
    values: (number[] | Float32Array)[]
  ): AttentionOutput {
    const raw = this.inner.compute(
      toFloat32Array(query),
      toFloat32Arrays(keys),
      toFloat32Arrays(values)
    );
    return {
      values: fromFloat32Array(raw),
      raw
    };
  }

  computeRaw(
    query: Float32Array,
    keys: Float32Array[],
    values: Float32Array[]
  ): Float32Array {
    return this.inner.compute(query, keys, values);
  }
}

/**
 * MoE configuration
 */
export interface MoEConfig {
  dim: number;
  numExperts: number;
  topK: number;
  expertCapacity?: number;
}

/**
 * Mixture of Experts attention
 */
export class MoEAttention {
  private inner: any;
  public readonly config: MoEConfig;

  /**
   * Create a new MoE attention instance
   *
   * @param config - MoE configuration
   */
  constructor(config: MoEConfig) {
    const attention = getAttentionModule();
    this.inner = new attention.MoEAttention({
      dim: config.dim,
      num_experts: config.numExperts,
      top_k: config.topK,
      expert_capacity: config.expertCapacity ?? 1.25,
    });
    this.config = config;
  }

  /**
   * Create with simple parameters
   */
  static simple(dim: number, numExperts: number, topK: number): MoEAttention {
    return new MoEAttention({ dim, numExperts, topK });
  }

  /**
   * Compute MoE attention
   */
  compute(
    query: number[] | Float32Array,
    keys: (number[] | Float32Array)[],
    values: (number[] | Float32Array)[]
  ): AttentionOutput {
    const raw = this.inner.compute(
      toFloat32Array(query),
      toFloat32Arrays(keys),
      toFloat32Arrays(values)
    );
    return {
      values: fromFloat32Array(raw),
      raw
    };
  }

  computeRaw(
    query: Float32Array,
    keys: Float32Array[],
    values: Float32Array[]
  ): Float32Array {
    return this.inner.compute(query, keys, values);
  }
}

// Hyperbolic math utilities

/**
 * Project a vector into the Poincare ball
 */
export function projectToPoincareBall(
  vector: number[] | Float32Array,
  curvature: number = 1.0
): number[] {
  const attention = getAttentionModule();
  const result = attention.projectToPoincareBall(toFloat32Array(vector), curvature);
  return fromFloat32Array(result);
}

/**
 * Compute hyperbolic (Poincare) distance between two points
 */
export function poincareDistance(
  a: number[] | Float32Array,
  b: number[] | Float32Array,
  curvature: number = 1.0
): number {
  const attention = getAttentionModule();
  return attention.poincareDistance(toFloat32Array(a), toFloat32Array(b), curvature);
}

/**
 * Mobius addition in hyperbolic space
 */
export function mobiusAddition(
  a: number[] | Float32Array,
  b: number[] | Float32Array,
  curvature: number = 1.0
): number[] {
  const attention = getAttentionModule();
  const result = attention.mobiusAddition(toFloat32Array(a), toFloat32Array(b), curvature);
  return fromFloat32Array(result);
}

/**
 * Exponential map from tangent space to hyperbolic space
 */
export function expMap(
  base: number[] | Float32Array,
  tangent: number[] | Float32Array,
  curvature: number = 1.0
): number[] {
  const attention = getAttentionModule();
  const result = attention.expMap(toFloat32Array(base), toFloat32Array(tangent), curvature);
  return fromFloat32Array(result);
}

/**
 * Logarithmic map from hyperbolic space to tangent space
 */
export function logMap(
  base: number[] | Float32Array,
  point: number[] | Float32Array,
  curvature: number = 1.0
): number[] {
  const attention = getAttentionModule();
  const result = attention.logMap(toFloat32Array(base), toFloat32Array(point), curvature);
  return fromFloat32Array(result);
}

/**
 * Check if attention module is available
 */
export function isAttentionAvailable(): boolean {
  try {
    getAttentionModule();
    return true;
  } catch {
    return false;
  }
}

/**
 * Get attention module version
 */
export function getAttentionVersion(): string | null {
  try {
    const attention = getAttentionModule();
    return attention.version?.() ?? null;
  } catch {
    return null;
  }
}

// ============================================================================
// Graph-based Attention (for code structure)
// ============================================================================

/**
 * Graph attention with Rotary Position Embeddings
 * Excellent for code AST and dependency graphs
 */
export class GraphRoPeAttention {
  private inner: any;
  public readonly dim: number;
  public readonly numHeads: number;
  public readonly maxSeqLen: number;

  constructor(dim: number, numHeads: number = 4, maxSeqLen: number = 4096) {
    const attention = getAttentionModule();
    this.inner = new attention.GraphRoPeAttention(dim, numHeads, maxSeqLen);
    this.dim = dim;
    this.numHeads = numHeads;
    this.maxSeqLen = maxSeqLen;
  }

  compute(
    query: number[] | Float32Array,
    keys: (number[] | Float32Array)[],
    values: (number[] | Float32Array)[],
    positions?: number[]
  ): AttentionOutput {
    const raw = this.inner.compute(
      toFloat32Array(query),
      toFloat32Arrays(keys),
      toFloat32Arrays(values),
      positions ? new Int32Array(positions) : undefined
    );
    return { values: fromFloat32Array(raw), raw };
  }
}

/**
 * Edge-featured attention for graphs with edge attributes
 * Useful for weighted dependency graphs
 */
export class EdgeFeaturedAttention {
  private inner: any;
  public readonly dim: number;
  public readonly edgeDim: number;

  constructor(dim: number, edgeDim: number = 16) {
    const attention = getAttentionModule();
    this.inner = new attention.EdgeFeaturedAttention(dim, edgeDim);
    this.dim = dim;
    this.edgeDim = edgeDim;
  }

  compute(
    query: number[] | Float32Array,
    keys: (number[] | Float32Array)[],
    values: (number[] | Float32Array)[],
    edgeFeatures?: (number[] | Float32Array)[]
  ): AttentionOutput {
    const raw = this.inner.compute(
      toFloat32Array(query),
      toFloat32Arrays(keys),
      toFloat32Arrays(values),
      edgeFeatures ? toFloat32Arrays(edgeFeatures) : undefined
    );
    return { values: fromFloat32Array(raw), raw };
  }
}

/**
 * Dual-space attention (Euclidean + Hyperbolic)
 * Best of both worlds for hierarchical + semantic similarity
 */
export class DualSpaceAttention {
  private inner: any;
  public readonly dim: number;
  public readonly curvature: number;
  public readonly alpha: number;

  constructor(dim: number, curvature: number = 1.0, alpha: number = 0.5) {
    const attention = getAttentionModule();
    this.inner = new attention.DualSpaceAttention(dim, curvature, alpha);
    this.dim = dim;
    this.curvature = curvature;
    this.alpha = alpha;
  }

  compute(
    query: number[] | Float32Array,
    keys: (number[] | Float32Array)[],
    values: (number[] | Float32Array)[]
  ): AttentionOutput {
    const raw = this.inner.compute(
      toFloat32Array(query),
      toFloat32Arrays(keys),
      toFloat32Arrays(values)
    );
    return { values: fromFloat32Array(raw), raw };
  }
}

/**
 * Basic dot-product attention
 */
export class DotProductAttention {
  private inner: any;
  public readonly dim: number;

  constructor(dim: number) {
    const attention = getAttentionModule();
    this.inner = new attention.DotProductAttention(dim);
    this.dim = dim;
  }

  compute(
    query: number[] | Float32Array,
    keys: (number[] | Float32Array)[],
    values: (number[] | Float32Array)[]
  ): AttentionOutput {
    const raw = this.inner.compute(
      toFloat32Array(query),
      toFloat32Arrays(keys),
      toFloat32Arrays(values)
    );
    return { values: fromFloat32Array(raw), raw };
  }
}

// ============================================================================
// Parallel/Batch Attention Compute
// ============================================================================

/**
 * Compute attention in parallel across multiple queries
 */
export async function parallelAttentionCompute(
  queries: (number[] | Float32Array)[],
  keys: (number[] | Float32Array)[],
  values: (number[] | Float32Array)[],
  attentionType: 'dot' | 'multi-head' | 'flash' | 'hyperbolic' | 'linear' = 'multi-head'
): Promise<number[][]> {
  const attention = getAttentionModule();
  const results = await attention.parallelAttentionCompute(
    toFloat32Arrays(queries),
    toFloat32Arrays(keys),
    toFloat32Arrays(values),
    attentionType
  );
  return results.map((r: Float32Array) => fromFloat32Array(r));
}

/**
 * Batch attention compute for multiple query-key-value sets
 */
export async function batchAttentionCompute(
  batches: Array<{
    query: number[] | Float32Array;
    keys: (number[] | Float32Array)[];
    values: (number[] | Float32Array)[];
  }>,
  attentionType: 'dot' | 'multi-head' | 'flash' | 'hyperbolic' | 'linear' = 'multi-head'
): Promise<number[][]> {
  const attention = getAttentionModule();
  const nativeBatches = batches.map(b => ({
    query: toFloat32Array(b.query),
    keys: toFloat32Arrays(b.keys),
    values: toFloat32Arrays(b.values),
  }));
  const results = await attention.batchAttentionCompute(nativeBatches, attentionType);
  return results.map((r: Float32Array) => fromFloat32Array(r));
}

/**
 * Async flash attention with callback
 */
export function computeFlashAttentionAsync(
  query: number[] | Float32Array,
  keys: (number[] | Float32Array)[],
  values: (number[] | Float32Array)[]
): Promise<number[]> {
  const attention = getAttentionModule();
  return new Promise((resolve, reject) => {
    attention.computeFlashAttentionAsync(
      toFloat32Array(query),
      toFloat32Arrays(keys),
      toFloat32Arrays(values),
      (err: Error | null, result: Float32Array) => {
        if (err) reject(err);
        else resolve(fromFloat32Array(result));
      }
    );
  });
}

/**
 * Async hyperbolic attention
 */
export function computeHyperbolicAttentionAsync(
  query: number[] | Float32Array,
  keys: (number[] | Float32Array)[],
  values: (number[] | Float32Array)[],
  curvature: number = 1.0
): Promise<number[]> {
  const attention = getAttentionModule();
  return new Promise((resolve, reject) => {
    attention.computeHyperbolicAttentionAsync(
      toFloat32Array(query),
      toFloat32Arrays(keys),
      toFloat32Arrays(values),
      curvature,
      (err: Error | null, result: Float32Array) => {
        if (err) reject(err);
        else resolve(fromFloat32Array(result));
      }
    );
  });
}

// ============================================================================
// Training Utilities (for SONA integration)
// ============================================================================

/**
 * Adam optimizer for attention training
 */
export class AdamOptimizer {
  private inner: any;

  constructor(learningRate: number = 0.001, beta1: number = 0.9, beta2: number = 0.999) {
    const attention = getAttentionModule();
    this.inner = new attention.AdamOptimizer(learningRate, beta1, beta2);
  }

  step(gradients: number[] | Float32Array, params: number[] | Float32Array): number[] {
    const result = this.inner.step(toFloat32Array(gradients), toFloat32Array(params));
    return fromFloat32Array(result);
  }
}

/**
 * InfoNCE contrastive loss
 */
export function infoNceLoss(
  anchor: number[] | Float32Array,
  positive: number[] | Float32Array,
  negatives: (number[] | Float32Array)[],
  temperature: number = 0.07
): number {
  const attention = getAttentionModule();
  return attention.InfoNceLoss.compute(
    toFloat32Array(anchor),
    toFloat32Array(positive),
    toFloat32Arrays(negatives),
    temperature
  );
}

/**
 * Hard negative mining for contrastive learning
 */
export function mineHardNegatives(
  anchor: number[] | Float32Array,
  candidates: (number[] | Float32Array)[],
  topK: number = 5
): number[][] {
  const attention = getAttentionModule();
  const miner = new attention.HardNegativeMiner(topK);
  const results = miner.mine(toFloat32Array(anchor), toFloat32Arrays(candidates));
  return results.map((r: Float32Array) => fromFloat32Array(r));
}

// ============================================================================
// Benchmarking
// ============================================================================

/**
 * Benchmark attention implementations
 */
export async function benchmarkAttention(
  dim: number,
  seqLen: number,
  iterations: number = 100
): Promise<Record<string, { avgMs: number; minMs: number; maxMs: number }>> {
  const attention = getAttentionModule();
  return attention.benchmarkAttention(dim, seqLen, iterations);
}

export default {
  // Core attention types
  DotProductAttention,
  MultiHeadAttention,
  FlashAttention,
  HyperbolicAttention,
  LinearAttention,
  LocalGlobalAttention,
  MoEAttention,

  // Graph attention types
  GraphRoPeAttention,
  EdgeFeaturedAttention,
  DualSpaceAttention,

  // Parallel/batch compute
  parallelAttentionCompute,
  batchAttentionCompute,
  computeFlashAttentionAsync,
  computeHyperbolicAttentionAsync,

  // Training utilities
  AdamOptimizer,
  infoNceLoss,
  mineHardNegatives,

  // Hyperbolic math
  projectToPoincareBall,
  poincareDistance,
  mobiusAddition,
  expMap,
  logMap,

  // Utilities
  isAttentionAvailable,
  getAttentionVersion,
  benchmarkAttention,
};
