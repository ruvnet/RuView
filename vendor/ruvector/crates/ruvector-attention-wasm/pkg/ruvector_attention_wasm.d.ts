/* tslint:disable */
/* eslint-disable */

/**
 * Adam optimizer
 */
export class WasmAdam {
    free(): void;
    [Symbol.dispose](): void;
    /**
     * Create a new Adam optimizer
     *
     * # Arguments
     * * `param_count` - Number of parameters
     * * `learning_rate` - Learning rate
     */
    constructor(param_count: number, learning_rate: number);
    /**
     * Reset optimizer state
     */
    reset(): void;
    /**
     * Perform optimization step
     *
     * # Arguments
     * * `params` - Current parameter values (will be updated in-place)
     * * `gradients` - Gradient values
     */
    step(params: Float32Array, gradients: Float32Array): void;
    /**
     * Get current learning rate
     */
    learning_rate: number;
}

/**
 * AdamW optimizer (Adam with decoupled weight decay)
 */
export class WasmAdamW {
    free(): void;
    [Symbol.dispose](): void;
    /**
     * Create a new AdamW optimizer
     *
     * # Arguments
     * * `param_count` - Number of parameters
     * * `learning_rate` - Learning rate
     * * `weight_decay` - Weight decay coefficient
     */
    constructor(param_count: number, learning_rate: number, weight_decay: number);
    /**
     * Reset optimizer state
     */
    reset(): void;
    /**
     * Perform optimization step with weight decay
     */
    step(params: Float32Array, gradients: Float32Array): void;
    /**
     * Get current learning rate
     */
    learning_rate: number;
    /**
     * Get weight decay
     */
    readonly weight_decay: number;
}

/**
 * Flash attention mechanism
 */
export class WasmFlashAttention {
    free(): void;
    [Symbol.dispose](): void;
    /**
     * Compute flash attention
     */
    compute(query: Float32Array, keys: any, values: any): Float32Array;
    /**
     * Create a new flash attention instance
     *
     * # Arguments
     * * `dim` - Embedding dimension
     * * `block_size` - Block size for tiling
     */
    constructor(dim: number, block_size: number);
}

/**
 * Hyperbolic attention mechanism
 */
export class WasmHyperbolicAttention {
    free(): void;
    [Symbol.dispose](): void;
    /**
     * Compute hyperbolic attention
     */
    compute(query: Float32Array, keys: any, values: any): Float32Array;
    /**
     * Create a new hyperbolic attention instance
     *
     * # Arguments
     * * `dim` - Embedding dimension
     * * `curvature` - Hyperbolic curvature parameter
     */
    constructor(dim: number, curvature: number);
    /**
     * Get the curvature
     */
    readonly curvature: number;
}

/**
 * InfoNCE contrastive loss for training
 */
export class WasmInfoNCELoss {
    free(): void;
    [Symbol.dispose](): void;
    /**
     * Compute InfoNCE loss
     *
     * # Arguments
     * * `anchor` - Anchor embedding
     * * `positive` - Positive example embedding
     * * `negatives` - Array of negative example embeddings
     */
    compute(anchor: Float32Array, positive: Float32Array, negatives: any): number;
    /**
     * Create a new InfoNCE loss instance
     *
     * # Arguments
     * * `temperature` - Temperature parameter for softmax
     */
    constructor(temperature: number);
}

/**
 * Learning rate scheduler
 */
export class WasmLRScheduler {
    free(): void;
    [Symbol.dispose](): void;
    /**
     * Get learning rate for current step
     */
    get_lr(): number;
    /**
     * Create a new learning rate scheduler with warmup and cosine decay
     *
     * # Arguments
     * * `initial_lr` - Initial learning rate
     * * `warmup_steps` - Number of warmup steps
     * * `total_steps` - Total training steps
     */
    constructor(initial_lr: number, warmup_steps: number, total_steps: number);
    /**
     * Reset scheduler
     */
    reset(): void;
    /**
     * Advance to next step
     */
    step(): void;
}

/**
 * Linear attention (Performer-style)
 */
export class WasmLinearAttention {
    free(): void;
    [Symbol.dispose](): void;
    /**
     * Compute linear attention
     */
    compute(query: Float32Array, keys: any, values: any): Float32Array;
    /**
     * Create a new linear attention instance
     *
     * # Arguments
     * * `dim` - Embedding dimension
     * * `num_features` - Number of random features
     */
    constructor(dim: number, num_features: number);
}

/**
 * Local-global attention mechanism
 */
export class WasmLocalGlobalAttention {
    free(): void;
    [Symbol.dispose](): void;
    /**
     * Compute local-global attention
     */
    compute(query: Float32Array, keys: any, values: any): Float32Array;
    /**
     * Create a new local-global attention instance
     *
     * # Arguments
     * * `dim` - Embedding dimension
     * * `local_window` - Size of local attention window
     * * `global_tokens` - Number of global attention tokens
     */
    constructor(dim: number, local_window: number, global_tokens: number);
}

/**
 * Mixture of Experts (MoE) attention
 */
export class WasmMoEAttention {
    free(): void;
    [Symbol.dispose](): void;
    /**
     * Compute MoE attention
     */
    compute(query: Float32Array, keys: any, values: any): Float32Array;
    /**
     * Create a new MoE attention instance
     *
     * # Arguments
     * * `dim` - Embedding dimension
     * * `num_experts` - Number of expert attention mechanisms
     * * `top_k` - Number of experts to use per query
     */
    constructor(dim: number, num_experts: number, top_k: number);
}

/**
 * Multi-head attention mechanism
 */
export class WasmMultiHeadAttention {
    free(): void;
    [Symbol.dispose](): void;
    /**
     * Compute multi-head attention
     */
    compute(query: Float32Array, keys: any, values: any): Float32Array;
    /**
     * Create a new multi-head attention instance
     *
     * # Arguments
     * * `dim` - Embedding dimension
     * * `num_heads` - Number of attention heads
     */
    constructor(dim: number, num_heads: number);
    /**
     * Get the dimension
     */
    readonly dim: number;
    /**
     * Get the number of heads
     */
    readonly num_heads: number;
}

/**
 * SGD optimizer with momentum
 */
export class WasmSGD {
    free(): void;
    [Symbol.dispose](): void;
    /**
     * Create a new SGD optimizer
     *
     * # Arguments
     * * `param_count` - Number of parameters
     * * `learning_rate` - Learning rate
     * * `momentum` - Momentum coefficient (default: 0)
     */
    constructor(param_count: number, learning_rate: number, momentum?: number | null);
    /**
     * Reset optimizer state
     */
    reset(): void;
    /**
     * Perform optimization step
     */
    step(params: Float32Array, gradients: Float32Array): void;
    /**
     * Get current learning rate
     */
    learning_rate: number;
}

/**
 * Compute attention weights from scores
 */
export function attention_weights(scores: Float32Array, temperature?: number | null): void;

/**
 * Get information about available attention mechanisms
 */
export function available_mechanisms(): any;

/**
 * Batch normalize vectors
 */
export function batch_normalize(vectors: any, epsilon?: number | null): Float32Array;

/**
 * Compute cosine similarity between two vectors
 */
export function cosine_similarity(a: Float32Array, b: Float32Array): number;

/**
 * Initialize the WASM module with panic hook
 */
export function init(): void;

/**
 * Compute L2 norm of a vector
 */
export function l2_norm(vec: Float32Array): number;

/**
 * Log a message to the browser console
 */
export function log(message: string): void;

/**
 * Log an error to the browser console
 */
export function log_error(message: string): void;

/**
 * Normalize a vector to unit length
 */
export function normalize(vec: Float32Array): void;

/**
 * Compute pairwise distances between vectors
 */
export function pairwise_distances(vectors: any): Float32Array;

/**
 * Generate random orthogonal matrix (for initialization)
 */
export function random_orthogonal_matrix(dim: number): Float32Array;

/**
 * Compute scaled dot-product attention
 *
 * # Arguments
 * * `query` - Query vector as Float32Array
 * * `keys` - Array of key vectors
 * * `values` - Array of value vectors
 * * `scale` - Optional scaling factor (defaults to 1/sqrt(dim))
 */
export function scaled_dot_attention(query: Float32Array, keys: any, values: any, scale?: number | null): Float32Array;

/**
 * Compute softmax of a vector
 */
export function softmax(vec: Float32Array): void;

/**
 * Get the version of the ruvector-attention-wasm crate
 */
export function version(): string;
