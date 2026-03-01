/**
 * GNN Wrapper - Safe wrapper around @ruvector/gnn with automatic array conversion
 *
 * This wrapper handles the array type conversion automatically, allowing users
 * to pass either regular arrays or Float32Arrays.
 *
 * The native @ruvector/gnn requires Float32Array for maximum performance.
 * This wrapper converts any input type to Float32Array automatically.
 *
 * Performance Tips:
 * - Pass Float32Array directly for zero-copy performance
 * - Use toFloat32Array/toFloat32ArrayBatch for pre-conversion
 * - Avoid repeated conversions in hot paths
 */
/**
 * Convert any array-like input to Float32Array (native requires Float32Array)
 * Optimized paths:
 * - Float32Array: zero-copy return
 * - Float64Array: efficient typed array copy
 * - Array: direct Float32Array construction
 */
export declare function toFloat32Array(input: number[] | Float32Array | Float64Array): Float32Array;
/**
 * Convert array of arrays to array of Float32Arrays
 */
export declare function toFloat32ArrayBatch(input: (number[] | Float32Array | Float64Array)[]): Float32Array[];
/**
 * Search result from differentiable search
 */
export interface DifferentiableSearchResult {
    /** Indices of top-k candidates */
    indices: number[];
    /** Soft weights for top-k candidates */
    weights: number[];
}
/**
 * Differentiable search using soft attention mechanism
 *
 * This wrapper automatically converts Float32Array inputs to regular arrays.
 *
 * @param query - Query vector (array or Float32Array)
 * @param candidates - List of candidate vectors (arrays or Float32Arrays)
 * @param k - Number of top results to return
 * @param temperature - Temperature for softmax (lower = sharper, higher = smoother)
 * @returns Search result with indices and soft weights
 *
 * @example
 * ```typescript
 * import { differentiableSearch } from 'ruvector/core/gnn-wrapper';
 *
 * // Works with regular arrays (auto-converted to Float32Array)
 * const result1 = differentiableSearch([1, 0, 0], [[1, 0, 0], [0, 1, 0]], 2, 1.0);
 *
 * // For best performance, use Float32Array directly (zero-copy)
 * const query = new Float32Array([1, 0, 0]);
 * const candidates = [new Float32Array([1, 0, 0]), new Float32Array([0, 1, 0])];
 * const result2 = differentiableSearch(query, candidates, 2, 1.0);
 * ```
 */
export declare function differentiableSearch(query: number[] | Float32Array | Float64Array, candidates: (number[] | Float32Array | Float64Array)[], k: number, temperature?: number): DifferentiableSearchResult;
/**
 * GNN Layer for HNSW topology
 */
export declare class RuvectorLayer {
    private inner;
    /**
     * Create a new Ruvector GNN layer
     *
     * @param inputDim - Dimension of input node embeddings
     * @param hiddenDim - Dimension of hidden representations
     * @param heads - Number of attention heads
     * @param dropout - Dropout rate (0.0 to 1.0)
     */
    constructor(inputDim: number, hiddenDim: number, heads: number, dropout?: number);
    /**
     * Forward pass through the GNN layer
     *
     * @param nodeEmbedding - Current node's embedding
     * @param neighborEmbeddings - Embeddings of neighbor nodes
     * @param edgeWeights - Weights of edges to neighbors
     * @returns Updated node embedding as Float32Array
     */
    forward(nodeEmbedding: number[] | Float32Array, neighborEmbeddings: (number[] | Float32Array)[], edgeWeights: number[] | Float32Array): Float32Array;
    /**
     * Serialize the layer to JSON
     */
    toJson(): string;
    /**
     * Deserialize the layer from JSON
     */
    static fromJson(json: string): RuvectorLayer;
}
/**
 * Tensor compressor with adaptive level selection
 */
export declare class TensorCompress {
    private inner;
    constructor();
    /**
     * Compress an embedding based on access frequency
     *
     * @param embedding - Input embedding vector
     * @param accessFreq - Access frequency (0.0 to 1.0)
     * @returns Compressed tensor as JSON string
     */
    compress(embedding: number[] | Float32Array, accessFreq: number): string;
    /**
     * Decompress a compressed tensor
     *
     * @param compressedJson - Compressed tensor JSON
     * @returns Decompressed embedding
     */
    decompress(compressedJson: string): number[];
}
/**
 * Hierarchical forward pass through GNN layers
 *
 * @param query - Query vector
 * @param layerEmbeddings - Embeddings organized by layer
 * @param gnnLayersJson - JSON array of serialized GNN layers
 * @returns Final embedding after hierarchical processing as Float32Array
 */
export declare function hierarchicalForward(query: number[] | Float32Array, layerEmbeddings: (number[] | Float32Array)[][], gnnLayersJson: string[]): Float32Array;
/**
 * Get compression level for a given access frequency
 */
export declare function getCompressionLevel(accessFreq: number): string;
/**
 * Check if GNN module is available
 */
export declare function isGnnAvailable(): boolean;
declare const _default: {
    differentiableSearch: typeof differentiableSearch;
    RuvectorLayer: typeof RuvectorLayer;
    TensorCompress: typeof TensorCompress;
    hierarchicalForward: typeof hierarchicalForward;
    getCompressionLevel: typeof getCompressionLevel;
    isGnnAvailable: typeof isGnnAvailable;
    toFloat32Array: typeof toFloat32Array;
    toFloat32ArrayBatch: typeof toFloat32ArrayBatch;
};
export default _default;
//# sourceMappingURL=gnn-wrapper.d.ts.map