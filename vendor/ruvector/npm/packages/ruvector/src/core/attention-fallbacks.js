"use strict";
/**
 * Attention Fallbacks - Safe wrapper around @ruvector/attention with automatic array conversion
 *
 * This wrapper handles the array type conversion automatically, allowing users
 * to pass either regular arrays or Float32Arrays.
 *
 * @ruvector/attention requires Float32Array inputs.
 * This wrapper handles the conversion automatically.
 */
Object.defineProperty(exports, "__esModule", { value: true });
exports.AdamOptimizer = exports.DotProductAttention = exports.DualSpaceAttention = exports.EdgeFeaturedAttention = exports.GraphRoPeAttention = exports.MoEAttention = exports.LocalGlobalAttention = exports.LinearAttention = exports.HyperbolicAttention = exports.FlashAttention = exports.MultiHeadAttention = void 0;
exports.projectToPoincareBall = projectToPoincareBall;
exports.poincareDistance = poincareDistance;
exports.mobiusAddition = mobiusAddition;
exports.expMap = expMap;
exports.logMap = logMap;
exports.isAttentionAvailable = isAttentionAvailable;
exports.getAttentionVersion = getAttentionVersion;
exports.parallelAttentionCompute = parallelAttentionCompute;
exports.batchAttentionCompute = batchAttentionCompute;
exports.computeFlashAttentionAsync = computeFlashAttentionAsync;
exports.computeHyperbolicAttentionAsync = computeHyperbolicAttentionAsync;
exports.infoNceLoss = infoNceLoss;
exports.mineHardNegatives = mineHardNegatives;
exports.benchmarkAttention = benchmarkAttention;
// Lazy load to avoid import errors if not installed
let attentionModule = null;
let loadError = null;
function getAttentionModule() {
    if (attentionModule)
        return attentionModule;
    if (loadError)
        throw loadError;
    try {
        attentionModule = require('@ruvector/attention');
        return attentionModule;
    }
    catch (e) {
        loadError = new Error(`@ruvector/attention is not installed or failed to load: ${e.message}\n` +
            `Install with: npm install @ruvector/attention`);
        throw loadError;
    }
}
/**
 * Convert any array-like input to Float32Array
 */
function toFloat32Array(input) {
    if (input instanceof Float32Array) {
        return input;
    }
    return new Float32Array(input);
}
/**
 * Convert nested arrays to Float32Arrays
 */
function toFloat32Arrays(inputs) {
    return inputs.map(arr => toFloat32Array(arr));
}
/**
 * Convert Float32Array result back to regular array if needed
 */
function fromFloat32Array(input) {
    return Array.from(input);
}
/**
 * Multi-head attention mechanism
 *
 * This wrapper automatically converts array inputs to Float32Array.
 */
class MultiHeadAttention {
    /**
     * Create a new multi-head attention instance
     *
     * @param dim - Embedding dimension (must be divisible by numHeads)
     * @param numHeads - Number of attention heads
     */
    constructor(dim, numHeads) {
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
    compute(query, keys, values) {
        const raw = this.inner.compute(toFloat32Array(query), toFloat32Arrays(keys), toFloat32Arrays(values));
        return {
            values: fromFloat32Array(raw),
            raw
        };
    }
    /**
     * Compute and return raw Float32Array (faster, no conversion)
     */
    computeRaw(query, keys, values) {
        return this.inner.compute(query, keys, values);
    }
    get headDim() {
        return this.dim / this.numHeads;
    }
}
exports.MultiHeadAttention = MultiHeadAttention;
/**
 * Flash attention with tiled computation
 */
class FlashAttention {
    /**
     * Create a new flash attention instance
     *
     * @param dim - Embedding dimension
     * @param blockSize - Block size for tiled computation (default: 512)
     */
    constructor(dim, blockSize = 512) {
        const attention = getAttentionModule();
        this.inner = new attention.FlashAttention(dim, blockSize);
        this.dim = dim;
        this.blockSize = blockSize;
    }
    /**
     * Compute flash attention
     */
    compute(query, keys, values) {
        const raw = this.inner.compute(toFloat32Array(query), toFloat32Arrays(keys), toFloat32Arrays(values));
        return {
            values: fromFloat32Array(raw),
            raw
        };
    }
    computeRaw(query, keys, values) {
        return this.inner.compute(query, keys, values);
    }
}
exports.FlashAttention = FlashAttention;
/**
 * Hyperbolic attention in Poincare ball model
 */
class HyperbolicAttention {
    /**
     * Create a new hyperbolic attention instance
     *
     * @param dim - Embedding dimension
     * @param curvature - Hyperbolic curvature (typically 1.0)
     */
    constructor(dim, curvature = 1.0) {
        const attention = getAttentionModule();
        this.inner = new attention.HyperbolicAttention(dim, curvature);
        this.dim = dim;
        this.curvature = curvature;
    }
    /**
     * Compute hyperbolic attention
     */
    compute(query, keys, values) {
        const raw = this.inner.compute(toFloat32Array(query), toFloat32Arrays(keys), toFloat32Arrays(values));
        return {
            values: fromFloat32Array(raw),
            raw
        };
    }
    computeRaw(query, keys, values) {
        return this.inner.compute(query, keys, values);
    }
}
exports.HyperbolicAttention = HyperbolicAttention;
/**
 * Linear attention (Performer-style) with O(n) complexity
 */
class LinearAttention {
    /**
     * Create a new linear attention instance
     *
     * @param dim - Embedding dimension
     * @param numFeatures - Number of random features
     */
    constructor(dim, numFeatures) {
        const attention = getAttentionModule();
        this.inner = new attention.LinearAttention(dim, numFeatures);
        this.dim = dim;
        this.numFeatures = numFeatures;
    }
    /**
     * Compute linear attention
     */
    compute(query, keys, values) {
        const raw = this.inner.compute(toFloat32Array(query), toFloat32Arrays(keys), toFloat32Arrays(values));
        return {
            values: fromFloat32Array(raw),
            raw
        };
    }
    computeRaw(query, keys, values) {
        return this.inner.compute(query, keys, values);
    }
}
exports.LinearAttention = LinearAttention;
/**
 * Local-global attention (Longformer-style)
 */
class LocalGlobalAttention {
    /**
     * Create a new local-global attention instance
     *
     * @param dim - Embedding dimension
     * @param localWindow - Size of local attention window
     * @param globalTokens - Number of global attention tokens
     */
    constructor(dim, localWindow, globalTokens) {
        const attention = getAttentionModule();
        this.inner = new attention.LocalGlobalAttention(dim, localWindow, globalTokens);
        this.dim = dim;
        this.localWindow = localWindow;
        this.globalTokens = globalTokens;
    }
    /**
     * Compute local-global attention
     */
    compute(query, keys, values) {
        const raw = this.inner.compute(toFloat32Array(query), toFloat32Arrays(keys), toFloat32Arrays(values));
        return {
            values: fromFloat32Array(raw),
            raw
        };
    }
    computeRaw(query, keys, values) {
        return this.inner.compute(query, keys, values);
    }
}
exports.LocalGlobalAttention = LocalGlobalAttention;
/**
 * Mixture of Experts attention
 */
class MoEAttention {
    /**
     * Create a new MoE attention instance
     *
     * @param config - MoE configuration
     */
    constructor(config) {
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
    static simple(dim, numExperts, topK) {
        return new MoEAttention({ dim, numExperts, topK });
    }
    /**
     * Compute MoE attention
     */
    compute(query, keys, values) {
        const raw = this.inner.compute(toFloat32Array(query), toFloat32Arrays(keys), toFloat32Arrays(values));
        return {
            values: fromFloat32Array(raw),
            raw
        };
    }
    computeRaw(query, keys, values) {
        return this.inner.compute(query, keys, values);
    }
}
exports.MoEAttention = MoEAttention;
// Hyperbolic math utilities
/**
 * Project a vector into the Poincare ball
 */
function projectToPoincareBall(vector, curvature = 1.0) {
    const attention = getAttentionModule();
    const result = attention.projectToPoincareBall(toFloat32Array(vector), curvature);
    return fromFloat32Array(result);
}
/**
 * Compute hyperbolic (Poincare) distance between two points
 */
function poincareDistance(a, b, curvature = 1.0) {
    const attention = getAttentionModule();
    return attention.poincareDistance(toFloat32Array(a), toFloat32Array(b), curvature);
}
/**
 * Mobius addition in hyperbolic space
 */
function mobiusAddition(a, b, curvature = 1.0) {
    const attention = getAttentionModule();
    const result = attention.mobiusAddition(toFloat32Array(a), toFloat32Array(b), curvature);
    return fromFloat32Array(result);
}
/**
 * Exponential map from tangent space to hyperbolic space
 */
function expMap(base, tangent, curvature = 1.0) {
    const attention = getAttentionModule();
    const result = attention.expMap(toFloat32Array(base), toFloat32Array(tangent), curvature);
    return fromFloat32Array(result);
}
/**
 * Logarithmic map from hyperbolic space to tangent space
 */
function logMap(base, point, curvature = 1.0) {
    const attention = getAttentionModule();
    const result = attention.logMap(toFloat32Array(base), toFloat32Array(point), curvature);
    return fromFloat32Array(result);
}
/**
 * Check if attention module is available
 */
function isAttentionAvailable() {
    try {
        getAttentionModule();
        return true;
    }
    catch {
        return false;
    }
}
/**
 * Get attention module version
 */
function getAttentionVersion() {
    try {
        const attention = getAttentionModule();
        return attention.version?.() ?? null;
    }
    catch {
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
class GraphRoPeAttention {
    constructor(dim, numHeads = 4, maxSeqLen = 4096) {
        const attention = getAttentionModule();
        this.inner = new attention.GraphRoPeAttention(dim, numHeads, maxSeqLen);
        this.dim = dim;
        this.numHeads = numHeads;
        this.maxSeqLen = maxSeqLen;
    }
    compute(query, keys, values, positions) {
        const raw = this.inner.compute(toFloat32Array(query), toFloat32Arrays(keys), toFloat32Arrays(values), positions ? new Int32Array(positions) : undefined);
        return { values: fromFloat32Array(raw), raw };
    }
}
exports.GraphRoPeAttention = GraphRoPeAttention;
/**
 * Edge-featured attention for graphs with edge attributes
 * Useful for weighted dependency graphs
 */
class EdgeFeaturedAttention {
    constructor(dim, edgeDim = 16) {
        const attention = getAttentionModule();
        this.inner = new attention.EdgeFeaturedAttention(dim, edgeDim);
        this.dim = dim;
        this.edgeDim = edgeDim;
    }
    compute(query, keys, values, edgeFeatures) {
        const raw = this.inner.compute(toFloat32Array(query), toFloat32Arrays(keys), toFloat32Arrays(values), edgeFeatures ? toFloat32Arrays(edgeFeatures) : undefined);
        return { values: fromFloat32Array(raw), raw };
    }
}
exports.EdgeFeaturedAttention = EdgeFeaturedAttention;
/**
 * Dual-space attention (Euclidean + Hyperbolic)
 * Best of both worlds for hierarchical + semantic similarity
 */
class DualSpaceAttention {
    constructor(dim, curvature = 1.0, alpha = 0.5) {
        const attention = getAttentionModule();
        this.inner = new attention.DualSpaceAttention(dim, curvature, alpha);
        this.dim = dim;
        this.curvature = curvature;
        this.alpha = alpha;
    }
    compute(query, keys, values) {
        const raw = this.inner.compute(toFloat32Array(query), toFloat32Arrays(keys), toFloat32Arrays(values));
        return { values: fromFloat32Array(raw), raw };
    }
}
exports.DualSpaceAttention = DualSpaceAttention;
/**
 * Basic dot-product attention
 */
class DotProductAttention {
    constructor(dim) {
        const attention = getAttentionModule();
        this.inner = new attention.DotProductAttention(dim);
        this.dim = dim;
    }
    compute(query, keys, values) {
        const raw = this.inner.compute(toFloat32Array(query), toFloat32Arrays(keys), toFloat32Arrays(values));
        return { values: fromFloat32Array(raw), raw };
    }
}
exports.DotProductAttention = DotProductAttention;
// ============================================================================
// Parallel/Batch Attention Compute
// ============================================================================
/**
 * Compute attention in parallel across multiple queries
 */
async function parallelAttentionCompute(queries, keys, values, attentionType = 'multi-head') {
    const attention = getAttentionModule();
    const results = await attention.parallelAttentionCompute(toFloat32Arrays(queries), toFloat32Arrays(keys), toFloat32Arrays(values), attentionType);
    return results.map((r) => fromFloat32Array(r));
}
/**
 * Batch attention compute for multiple query-key-value sets
 */
async function batchAttentionCompute(batches, attentionType = 'multi-head') {
    const attention = getAttentionModule();
    const nativeBatches = batches.map(b => ({
        query: toFloat32Array(b.query),
        keys: toFloat32Arrays(b.keys),
        values: toFloat32Arrays(b.values),
    }));
    const results = await attention.batchAttentionCompute(nativeBatches, attentionType);
    return results.map((r) => fromFloat32Array(r));
}
/**
 * Async flash attention with callback
 */
function computeFlashAttentionAsync(query, keys, values) {
    const attention = getAttentionModule();
    return new Promise((resolve, reject) => {
        attention.computeFlashAttentionAsync(toFloat32Array(query), toFloat32Arrays(keys), toFloat32Arrays(values), (err, result) => {
            if (err)
                reject(err);
            else
                resolve(fromFloat32Array(result));
        });
    });
}
/**
 * Async hyperbolic attention
 */
function computeHyperbolicAttentionAsync(query, keys, values, curvature = 1.0) {
    const attention = getAttentionModule();
    return new Promise((resolve, reject) => {
        attention.computeHyperbolicAttentionAsync(toFloat32Array(query), toFloat32Arrays(keys), toFloat32Arrays(values), curvature, (err, result) => {
            if (err)
                reject(err);
            else
                resolve(fromFloat32Array(result));
        });
    });
}
// ============================================================================
// Training Utilities (for SONA integration)
// ============================================================================
/**
 * Adam optimizer for attention training
 */
class AdamOptimizer {
    constructor(learningRate = 0.001, beta1 = 0.9, beta2 = 0.999) {
        const attention = getAttentionModule();
        this.inner = new attention.AdamOptimizer(learningRate, beta1, beta2);
    }
    step(gradients, params) {
        const result = this.inner.step(toFloat32Array(gradients), toFloat32Array(params));
        return fromFloat32Array(result);
    }
}
exports.AdamOptimizer = AdamOptimizer;
/**
 * InfoNCE contrastive loss
 */
function infoNceLoss(anchor, positive, negatives, temperature = 0.07) {
    const attention = getAttentionModule();
    return attention.InfoNceLoss.compute(toFloat32Array(anchor), toFloat32Array(positive), toFloat32Arrays(negatives), temperature);
}
/**
 * Hard negative mining for contrastive learning
 */
function mineHardNegatives(anchor, candidates, topK = 5) {
    const attention = getAttentionModule();
    const miner = new attention.HardNegativeMiner(topK);
    const results = miner.mine(toFloat32Array(anchor), toFloat32Arrays(candidates));
    return results.map((r) => fromFloat32Array(r));
}
// ============================================================================
// Benchmarking
// ============================================================================
/**
 * Benchmark attention implementations
 */
async function benchmarkAttention(dim, seqLen, iterations = 100) {
    const attention = getAttentionModule();
    return attention.benchmarkAttention(dim, seqLen, iterations);
}
exports.default = {
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
//# sourceMappingURL=attention-fallbacks.js.map