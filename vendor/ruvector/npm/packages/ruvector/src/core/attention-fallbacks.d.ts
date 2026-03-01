/**
 * Attention Fallbacks - Safe wrapper around @ruvector/attention with automatic array conversion
 *
 * This wrapper handles the array type conversion automatically, allowing users
 * to pass either regular arrays or Float32Arrays.
 *
 * @ruvector/attention requires Float32Array inputs.
 * This wrapper handles the conversion automatically.
 */
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
export declare class MultiHeadAttention {
    private inner;
    readonly dim: number;
    readonly numHeads: number;
    /**
     * Create a new multi-head attention instance
     *
     * @param dim - Embedding dimension (must be divisible by numHeads)
     * @param numHeads - Number of attention heads
     */
    constructor(dim: number, numHeads: number);
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
    compute(query: number[] | Float32Array, keys: (number[] | Float32Array)[], values: (number[] | Float32Array)[]): AttentionOutput;
    /**
     * Compute and return raw Float32Array (faster, no conversion)
     */
    computeRaw(query: Float32Array, keys: Float32Array[], values: Float32Array[]): Float32Array;
    get headDim(): number;
}
/**
 * Flash attention with tiled computation
 */
export declare class FlashAttention {
    private inner;
    readonly dim: number;
    readonly blockSize: number;
    /**
     * Create a new flash attention instance
     *
     * @param dim - Embedding dimension
     * @param blockSize - Block size for tiled computation (default: 512)
     */
    constructor(dim: number, blockSize?: number);
    /**
     * Compute flash attention
     */
    compute(query: number[] | Float32Array, keys: (number[] | Float32Array)[], values: (number[] | Float32Array)[]): AttentionOutput;
    computeRaw(query: Float32Array, keys: Float32Array[], values: Float32Array[]): Float32Array;
}
/**
 * Hyperbolic attention in Poincare ball model
 */
export declare class HyperbolicAttention {
    private inner;
    readonly dim: number;
    readonly curvature: number;
    /**
     * Create a new hyperbolic attention instance
     *
     * @param dim - Embedding dimension
     * @param curvature - Hyperbolic curvature (typically 1.0)
     */
    constructor(dim: number, curvature?: number);
    /**
     * Compute hyperbolic attention
     */
    compute(query: number[] | Float32Array, keys: (number[] | Float32Array)[], values: (number[] | Float32Array)[]): AttentionOutput;
    computeRaw(query: Float32Array, keys: Float32Array[], values: Float32Array[]): Float32Array;
}
/**
 * Linear attention (Performer-style) with O(n) complexity
 */
export declare class LinearAttention {
    private inner;
    readonly dim: number;
    readonly numFeatures: number;
    /**
     * Create a new linear attention instance
     *
     * @param dim - Embedding dimension
     * @param numFeatures - Number of random features
     */
    constructor(dim: number, numFeatures: number);
    /**
     * Compute linear attention
     */
    compute(query: number[] | Float32Array, keys: (number[] | Float32Array)[], values: (number[] | Float32Array)[]): AttentionOutput;
    computeRaw(query: Float32Array, keys: Float32Array[], values: Float32Array[]): Float32Array;
}
/**
 * Local-global attention (Longformer-style)
 */
export declare class LocalGlobalAttention {
    private inner;
    readonly dim: number;
    readonly localWindow: number;
    readonly globalTokens: number;
    /**
     * Create a new local-global attention instance
     *
     * @param dim - Embedding dimension
     * @param localWindow - Size of local attention window
     * @param globalTokens - Number of global attention tokens
     */
    constructor(dim: number, localWindow: number, globalTokens: number);
    /**
     * Compute local-global attention
     */
    compute(query: number[] | Float32Array, keys: (number[] | Float32Array)[], values: (number[] | Float32Array)[]): AttentionOutput;
    computeRaw(query: Float32Array, keys: Float32Array[], values: Float32Array[]): Float32Array;
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
export declare class MoEAttention {
    private inner;
    readonly config: MoEConfig;
    /**
     * Create a new MoE attention instance
     *
     * @param config - MoE configuration
     */
    constructor(config: MoEConfig);
    /**
     * Create with simple parameters
     */
    static simple(dim: number, numExperts: number, topK: number): MoEAttention;
    /**
     * Compute MoE attention
     */
    compute(query: number[] | Float32Array, keys: (number[] | Float32Array)[], values: (number[] | Float32Array)[]): AttentionOutput;
    computeRaw(query: Float32Array, keys: Float32Array[], values: Float32Array[]): Float32Array;
}
/**
 * Project a vector into the Poincare ball
 */
export declare function projectToPoincareBall(vector: number[] | Float32Array, curvature?: number): number[];
/**
 * Compute hyperbolic (Poincare) distance between two points
 */
export declare function poincareDistance(a: number[] | Float32Array, b: number[] | Float32Array, curvature?: number): number;
/**
 * Mobius addition in hyperbolic space
 */
export declare function mobiusAddition(a: number[] | Float32Array, b: number[] | Float32Array, curvature?: number): number[];
/**
 * Exponential map from tangent space to hyperbolic space
 */
export declare function expMap(base: number[] | Float32Array, tangent: number[] | Float32Array, curvature?: number): number[];
/**
 * Logarithmic map from hyperbolic space to tangent space
 */
export declare function logMap(base: number[] | Float32Array, point: number[] | Float32Array, curvature?: number): number[];
/**
 * Check if attention module is available
 */
export declare function isAttentionAvailable(): boolean;
/**
 * Get attention module version
 */
export declare function getAttentionVersion(): string | null;
/**
 * Graph attention with Rotary Position Embeddings
 * Excellent for code AST and dependency graphs
 */
export declare class GraphRoPeAttention {
    private inner;
    readonly dim: number;
    readonly numHeads: number;
    readonly maxSeqLen: number;
    constructor(dim: number, numHeads?: number, maxSeqLen?: number);
    compute(query: number[] | Float32Array, keys: (number[] | Float32Array)[], values: (number[] | Float32Array)[], positions?: number[]): AttentionOutput;
}
/**
 * Edge-featured attention for graphs with edge attributes
 * Useful for weighted dependency graphs
 */
export declare class EdgeFeaturedAttention {
    private inner;
    readonly dim: number;
    readonly edgeDim: number;
    constructor(dim: number, edgeDim?: number);
    compute(query: number[] | Float32Array, keys: (number[] | Float32Array)[], values: (number[] | Float32Array)[], edgeFeatures?: (number[] | Float32Array)[]): AttentionOutput;
}
/**
 * Dual-space attention (Euclidean + Hyperbolic)
 * Best of both worlds for hierarchical + semantic similarity
 */
export declare class DualSpaceAttention {
    private inner;
    readonly dim: number;
    readonly curvature: number;
    readonly alpha: number;
    constructor(dim: number, curvature?: number, alpha?: number);
    compute(query: number[] | Float32Array, keys: (number[] | Float32Array)[], values: (number[] | Float32Array)[]): AttentionOutput;
}
/**
 * Basic dot-product attention
 */
export declare class DotProductAttention {
    private inner;
    readonly dim: number;
    constructor(dim: number);
    compute(query: number[] | Float32Array, keys: (number[] | Float32Array)[], values: (number[] | Float32Array)[]): AttentionOutput;
}
/**
 * Compute attention in parallel across multiple queries
 */
export declare function parallelAttentionCompute(queries: (number[] | Float32Array)[], keys: (number[] | Float32Array)[], values: (number[] | Float32Array)[], attentionType?: 'dot' | 'multi-head' | 'flash' | 'hyperbolic' | 'linear'): Promise<number[][]>;
/**
 * Batch attention compute for multiple query-key-value sets
 */
export declare function batchAttentionCompute(batches: Array<{
    query: number[] | Float32Array;
    keys: (number[] | Float32Array)[];
    values: (number[] | Float32Array)[];
}>, attentionType?: 'dot' | 'multi-head' | 'flash' | 'hyperbolic' | 'linear'): Promise<number[][]>;
/**
 * Async flash attention with callback
 */
export declare function computeFlashAttentionAsync(query: number[] | Float32Array, keys: (number[] | Float32Array)[], values: (number[] | Float32Array)[]): Promise<number[]>;
/**
 * Async hyperbolic attention
 */
export declare function computeHyperbolicAttentionAsync(query: number[] | Float32Array, keys: (number[] | Float32Array)[], values: (number[] | Float32Array)[], curvature?: number): Promise<number[]>;
/**
 * Adam optimizer for attention training
 */
export declare class AdamOptimizer {
    private inner;
    constructor(learningRate?: number, beta1?: number, beta2?: number);
    step(gradients: number[] | Float32Array, params: number[] | Float32Array): number[];
}
/**
 * InfoNCE contrastive loss
 */
export declare function infoNceLoss(anchor: number[] | Float32Array, positive: number[] | Float32Array, negatives: (number[] | Float32Array)[], temperature?: number): number;
/**
 * Hard negative mining for contrastive learning
 */
export declare function mineHardNegatives(anchor: number[] | Float32Array, candidates: (number[] | Float32Array)[], topK?: number): number[][];
/**
 * Benchmark attention implementations
 */
export declare function benchmarkAttention(dim: number, seqLen: number, iterations?: number): Promise<Record<string, {
    avgMs: number;
    minMs: number;
    maxMs: number;
}>>;
declare const _default: {
    DotProductAttention: typeof DotProductAttention;
    MultiHeadAttention: typeof MultiHeadAttention;
    FlashAttention: typeof FlashAttention;
    HyperbolicAttention: typeof HyperbolicAttention;
    LinearAttention: typeof LinearAttention;
    LocalGlobalAttention: typeof LocalGlobalAttention;
    MoEAttention: typeof MoEAttention;
    GraphRoPeAttention: typeof GraphRoPeAttention;
    EdgeFeaturedAttention: typeof EdgeFeaturedAttention;
    DualSpaceAttention: typeof DualSpaceAttention;
    parallelAttentionCompute: typeof parallelAttentionCompute;
    batchAttentionCompute: typeof batchAttentionCompute;
    computeFlashAttentionAsync: typeof computeFlashAttentionAsync;
    computeHyperbolicAttentionAsync: typeof computeHyperbolicAttentionAsync;
    AdamOptimizer: typeof AdamOptimizer;
    infoNceLoss: typeof infoNceLoss;
    mineHardNegatives: typeof mineHardNegatives;
    projectToPoincareBall: typeof projectToPoincareBall;
    poincareDistance: typeof poincareDistance;
    mobiusAddition: typeof mobiusAddition;
    expMap: typeof expMap;
    logMap: typeof logMap;
    isAttentionAvailable: typeof isAttentionAvailable;
    getAttentionVersion: typeof getAttentionVersion;
    benchmarkAttention: typeof benchmarkAttention;
};
export default _default;
//# sourceMappingURL=attention-fallbacks.d.ts.map