/**
 * SIMD Operations for vector computations
 *
 * Uses native SIMD instructions (AVX2/AVX512/SSE4.1/NEON) when available,
 * falls back to JavaScript implementations otherwise.
 */
/**
 * SIMD Operations class
 *
 * Provides hardware-accelerated vector operations when native module is available.
 *
 * @example
 * ```typescript
 * import { SimdOps } from '@ruvector/ruvllm';
 *
 * const simd = new SimdOps();
 *
 * // Compute dot product
 * const result = simd.dotProduct([1, 2, 3], [4, 5, 6]);
 * console.log(result); // 32
 *
 * // Check capabilities
 * console.log(simd.capabilities()); // ['AVX2', 'FMA']
 * ```
 */
export declare class SimdOps {
    private native;
    constructor();
    /**
     * Compute dot product of two vectors
     */
    dotProduct(a: number[], b: number[]): number;
    /**
     * Compute cosine similarity between two vectors
     */
    cosineSimilarity(a: number[], b: number[]): number;
    /**
     * Compute L2 (Euclidean) distance between two vectors
     */
    l2Distance(a: number[], b: number[]): number;
    /**
     * Matrix-vector multiplication
     */
    matvec(matrix: number[][], vector: number[]): number[];
    /**
     * Softmax activation function
     */
    softmax(input: number[]): number[];
    /**
     * Element-wise addition
     */
    add(a: number[], b: number[]): number[];
    /**
     * Element-wise multiplication
     */
    mul(a: number[], b: number[]): number[];
    /**
     * Scale vector by scalar
     */
    scale(a: number[], scalar: number): number[];
    /**
     * Normalize vector to unit length
     */
    normalize(a: number[]): number[];
    /**
     * ReLU activation
     */
    relu(input: number[]): number[];
    /**
     * GELU activation (approximate)
     */
    gelu(input: number[]): number[];
    /**
     * Sigmoid activation
     */
    sigmoid(input: number[]): number[];
    /**
     * Layer normalization
     */
    layerNorm(input: number[], eps?: number): number[];
    /**
     * Check if native SIMD is available
     */
    isNative(): boolean;
    /**
     * Get available SIMD capabilities
     */
    capabilities(): string[];
}
//# sourceMappingURL=simd.d.ts.map