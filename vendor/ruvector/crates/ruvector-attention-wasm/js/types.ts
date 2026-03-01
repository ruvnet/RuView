/**
 * TypeScript type definitions for ruvector-attention-wasm
 */

export interface AttentionConfig {
  /** Embedding dimension */
  dim: number;
  /** Number of attention heads (for multi-head attention) */
  numHeads?: number;
  /** Dropout probability */
  dropout?: number;
  /** Scaling factor for attention scores */
  scale?: number;
  /** Whether to use causal masking */
  causal?: boolean;
}

export interface MultiHeadConfig extends AttentionConfig {
  numHeads: number;
}

export interface HyperbolicConfig extends AttentionConfig {
  /** Hyperbolic space curvature */
  curvature: number;
}

export interface LinearAttentionConfig extends AttentionConfig {
  /** Number of random features for kernel approximation */
  numFeatures: number;
}

export interface FlashAttentionConfig extends AttentionConfig {
  /** Block size for tiling */
  blockSize: number;
}

export interface LocalGlobalConfig extends AttentionConfig {
  /** Size of local attention window */
  localWindow: number;
  /** Number of global attention tokens */
  globalTokens: number;
}

export interface MoEConfig extends AttentionConfig {
  /** Number of expert attention mechanisms */
  numExperts: number;
  /** Number of experts to use per query */
  topK: number;
  /** Maximum capacity per expert */
  expertCapacity?: number;
  /** Load balancing coefficient */
  balanceCoeff?: number;
}

export interface TrainingConfig {
  /** Learning rate for optimizer */
  learningRate: number;
  /** Temperature parameter for contrastive loss */
  temperature?: number;
  /** First moment decay rate (Adam/AdamW) */
  beta1?: number;
  /** Second moment decay rate (Adam/AdamW) */
  beta2?: number;
  /** Weight decay coefficient (AdamW) */
  weightDecay?: number;
  /** Numerical stability constant */
  epsilon?: number;
}

export interface SchedulerConfig {
  /** Initial learning rate */
  initialLR: number;
  /** Number of warmup steps */
  warmupSteps: number;
  /** Total training steps */
  totalSteps: number;
}

export interface ExpertStats {
  /** Number of times each expert was selected */
  selectionCounts: number[];
  /** Average load per expert */
  averageLoad: number[];
  /** Load balance factor (lower is better) */
  loadBalance: number;
}

/**
 * Attention mechanism types
 */
export type AttentionType =
  | 'scaled_dot_product'
  | 'multi_head'
  | 'hyperbolic'
  | 'linear'
  | 'flash'
  | 'local_global'
  | 'moe';

/**
 * Optimizer types
 */
export type OptimizerType = 'adam' | 'adamw';

/**
 * Loss function types
 */
export type LossType = 'info_nce';
