/**
 * TypeScript wrapper for ruvector-attention-wasm
 * Provides a clean, type-safe API for attention mechanisms
 */

import init, * as wasm from '../pkg/ruvector_attention_wasm';
import type {
  AttentionConfig,
  MultiHeadConfig,
  HyperbolicConfig,
  LinearAttentionConfig,
  FlashAttentionConfig,
  LocalGlobalConfig,
  MoEConfig,
  TrainingConfig,
  SchedulerConfig,
  ExpertStats,
  AttentionType,
} from './types';

export * from './types';

let initialized = false;

/**
 * Initialize the WASM module
 * Must be called before using any attention mechanisms
 */
export async function initialize(): Promise<void> {
  if (!initialized) {
    await init();
    initialized = true;
  }
}

/**
 * Get the version of the ruvector-attention-wasm package
 */
export function version(): string {
  return wasm.version();
}

/**
 * Get list of available attention mechanisms
 */
export function availableMechanisms(): AttentionType[] {
  return wasm.available_mechanisms() as AttentionType[];
}

/**
 * Multi-head attention mechanism
 */
export class MultiHeadAttention {
  private inner: wasm.WasmMultiHeadAttention;

  constructor(config: MultiHeadConfig) {
    this.inner = new wasm.WasmMultiHeadAttention(config.dim, config.numHeads);
  }

  /**
   * Compute multi-head attention
   */
  compute(query: Float32Array, keys: Float32Array[], values: Float32Array[]): Float32Array {
    const result = this.inner.compute(query, keys, values);
    return new Float32Array(result);
  }

  get numHeads(): number {
    return this.inner.num_heads;
  }

  get dim(): number {
    return this.inner.dim;
  }

  free(): void {
    this.inner.free();
  }
}

/**
 * Hyperbolic attention mechanism
 */
export class HyperbolicAttention {
  private inner: wasm.WasmHyperbolicAttention;

  constructor(config: HyperbolicConfig) {
    this.inner = new wasm.WasmHyperbolicAttention(config.dim, config.curvature);
  }

  compute(query: Float32Array, keys: Float32Array[], values: Float32Array[]): Float32Array {
    const result = this.inner.compute(query, keys, values);
    return new Float32Array(result);
  }

  get curvature(): number {
    return this.inner.curvature;
  }

  free(): void {
    this.inner.free();
  }
}

/**
 * Linear attention (Performer-style)
 */
export class LinearAttention {
  private inner: wasm.WasmLinearAttention;

  constructor(config: LinearAttentionConfig) {
    this.inner = new wasm.WasmLinearAttention(config.dim, config.numFeatures);
  }

  compute(query: Float32Array, keys: Float32Array[], values: Float32Array[]): Float32Array {
    const result = this.inner.compute(query, keys, values);
    return new Float32Array(result);
  }

  free(): void {
    this.inner.free();
  }
}

/**
 * Flash attention mechanism
 */
export class FlashAttention {
  private inner: wasm.WasmFlashAttention;

  constructor(config: FlashAttentionConfig) {
    this.inner = new wasm.WasmFlashAttention(config.dim, config.blockSize);
  }

  compute(query: Float32Array, keys: Float32Array[], values: Float32Array[]): Float32Array {
    const result = this.inner.compute(query, keys, values);
    return new Float32Array(result);
  }

  free(): void {
    this.inner.free();
  }
}

/**
 * Local-global attention mechanism
 */
export class LocalGlobalAttention {
  private inner: wasm.WasmLocalGlobalAttention;

  constructor(config: LocalGlobalConfig) {
    this.inner = new wasm.WasmLocalGlobalAttention(
      config.dim,
      config.localWindow,
      config.globalTokens
    );
  }

  compute(query: Float32Array, keys: Float32Array[], values: Float32Array[]): Float32Array {
    const result = this.inner.compute(query, keys, values);
    return new Float32Array(result);
  }

  free(): void {
    this.inner.free();
  }
}

/**
 * Mixture of Experts attention
 */
export class MoEAttention {
  private inner: wasm.WasmMoEAttention;

  constructor(config: MoEConfig) {
    this.inner = new wasm.WasmMoEAttention(config.dim, config.numExperts, config.topK);
  }

  compute(query: Float32Array, keys: Float32Array[], values: Float32Array[]): Float32Array {
    const result = this.inner.compute(query, keys, values);
    return new Float32Array(result);
  }

  getExpertStats(): ExpertStats {
    return this.inner.expert_stats() as ExpertStats;
  }

  free(): void {
    this.inner.free();
  }
}

/**
 * InfoNCE contrastive loss
 */
export class InfoNCELoss {
  private inner: wasm.WasmInfoNCELoss;

  constructor(temperature: number = 0.07) {
    this.inner = new wasm.WasmInfoNCELoss(temperature);
  }

  compute(anchor: Float32Array, positive: Float32Array, negatives: Float32Array[]): number {
    return this.inner.compute(anchor, positive, negatives);
  }

  computeMultiPositive(
    anchor: Float32Array,
    positives: Float32Array[],
    negatives: Float32Array[]
  ): number {
    return this.inner.compute_multi_positive(anchor, positives, negatives);
  }

  free(): void {
    this.inner.free();
  }
}

/**
 * Adam optimizer
 */
export class Adam {
  private inner: wasm.WasmAdam;

  constructor(paramCount: number, config: TrainingConfig) {
    this.inner = new wasm.WasmAdam(
      paramCount,
      config.learningRate,
      config.beta1,
      config.beta2,
      config.epsilon
    );
  }

  step(params: Float32Array, gradients: Float32Array): void {
    this.inner.step(params, gradients);
  }

  reset(): void {
    this.inner.reset();
  }

  get learningRate(): number {
    return this.inner.learning_rate;
  }

  set learningRate(lr: number) {
    this.inner.learning_rate = lr;
  }

  free(): void {
    this.inner.free();
  }
}

/**
 * AdamW optimizer (Adam with decoupled weight decay)
 */
export class AdamW {
  private inner: wasm.WasmAdamW;

  constructor(paramCount: number, config: TrainingConfig) {
    if (!config.weightDecay) {
      throw new Error('AdamW requires weightDecay parameter');
    }

    this.inner = new wasm.WasmAdamW(
      paramCount,
      config.learningRate,
      config.weightDecay,
      config.beta1,
      config.beta2,
      config.epsilon
    );
  }

  step(params: Float32Array, gradients: Float32Array): void {
    this.inner.step(params, gradients);
  }

  reset(): void {
    this.inner.reset();
  }

  get learningRate(): number {
    return this.inner.learning_rate;
  }

  set learningRate(lr: number) {
    this.inner.learning_rate = lr;
  }

  get weightDecay(): number {
    return this.inner.weight_decay;
  }

  free(): void {
    this.inner.free();
  }
}

/**
 * Learning rate scheduler with warmup and cosine decay
 */
export class LRScheduler {
  private inner: wasm.WasmLRScheduler;

  constructor(config: SchedulerConfig) {
    this.inner = new wasm.WasmLRScheduler(
      config.initialLR,
      config.warmupSteps,
      config.totalSteps
    );
  }

  getLR(): number {
    return this.inner.get_lr();
  }

  step(): void {
    this.inner.step();
  }

  reset(): void {
    this.inner.reset();
  }

  free(): void {
    this.inner.free();
  }
}

/**
 * Utility functions
 */
export const utils = {
  /**
   * Compute cosine similarity between two vectors
   */
  cosineSimilarity(a: Float32Array, b: Float32Array): number {
    return wasm.cosine_similarity(a, b);
  },

  /**
   * Compute L2 norm of a vector
   */
  l2Norm(vec: Float32Array): number {
    return wasm.l2_norm(vec);
  },

  /**
   * Normalize a vector to unit length (in-place)
   */
  normalize(vec: Float32Array): void {
    wasm.normalize(vec);
  },

  /**
   * Apply softmax to a vector (in-place)
   */
  softmax(vec: Float32Array): void {
    wasm.softmax(vec);
  },

  /**
   * Compute attention weights from scores (in-place)
   */
  attentionWeights(scores: Float32Array, temperature?: number): void {
    wasm.attention_weights(scores, temperature);
  },

  /**
   * Batch normalize vectors
   */
  batchNormalize(vectors: Float32Array[], epsilon?: number): Float32Array {
    const result = wasm.batch_normalize(vectors, epsilon);
    return new Float32Array(result);
  },

  /**
   * Generate random orthogonal matrix
   */
  randomOrthogonalMatrix(dim: number): Float32Array {
    const result = wasm.random_orthogonal_matrix(dim);
    return new Float32Array(result);
  },

  /**
   * Compute pairwise distances between vectors
   */
  pairwiseDistances(vectors: Float32Array[]): Float32Array {
    const result = wasm.pairwise_distances(vectors);
    return new Float32Array(result);
  },
};

/**
 * Simple scaled dot-product attention (functional API)
 */
export function scaledDotAttention(
  query: Float32Array,
  keys: Float32Array[],
  values: Float32Array[],
  scale?: number
): Float32Array {
  const result = wasm.scaled_dot_attention(query, keys, values, scale);
  return new Float32Array(result);
}

// Re-export WASM module for advanced usage
export { wasm };
