"use strict";
/**
 * SIMD Operations for vector computations
 *
 * Uses native SIMD instructions (AVX2/AVX512/SSE4.1/NEON) when available,
 * falls back to JavaScript implementations otherwise.
 */
Object.defineProperty(exports, "__esModule", { value: true });
exports.SimdOps = void 0;
const native_1 = require("./native");
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
class SimdOps {
    constructor() {
        this.native = null;
        const mod = (0, native_1.getNativeModule)();
        if (mod) {
            try {
                this.native = new mod.SimdOperations();
            }
            catch {
                // Fall back to JS implementation
            }
        }
    }
    /**
     * Compute dot product of two vectors
     */
    dotProduct(a, b) {
        if (this.native) {
            return this.native.dotProduct(a, b);
        }
        // JavaScript fallback
        let sum = 0;
        const len = Math.min(a.length, b.length);
        for (let i = 0; i < len; i++) {
            sum += a[i] * b[i];
        }
        return sum;
    }
    /**
     * Compute cosine similarity between two vectors
     */
    cosineSimilarity(a, b) {
        if (this.native) {
            return this.native.cosineSimilarity(a, b);
        }
        // JavaScript fallback
        let dot = 0;
        let normA = 0;
        let normB = 0;
        const len = Math.min(a.length, b.length);
        for (let i = 0; i < len; i++) {
            dot += a[i] * b[i];
            normA += a[i] * a[i];
            normB += b[i] * b[i];
        }
        const denom = Math.sqrt(normA) * Math.sqrt(normB);
        return denom > 0 ? dot / denom : 0;
    }
    /**
     * Compute L2 (Euclidean) distance between two vectors
     */
    l2Distance(a, b) {
        if (this.native) {
            return this.native.l2Distance(a, b);
        }
        // JavaScript fallback
        let sum = 0;
        const len = Math.min(a.length, b.length);
        for (let i = 0; i < len; i++) {
            const diff = a[i] - b[i];
            sum += diff * diff;
        }
        return Math.sqrt(sum);
    }
    /**
     * Matrix-vector multiplication
     */
    matvec(matrix, vector) {
        if (this.native) {
            return this.native.matvec(matrix, vector);
        }
        // JavaScript fallback
        return matrix.map(row => this.dotProduct(row, vector));
    }
    /**
     * Softmax activation function
     */
    softmax(input) {
        if (this.native) {
            return this.native.softmax(input);
        }
        // JavaScript fallback
        const max = Math.max(...input);
        const exps = input.map(x => Math.exp(x - max));
        const sum = exps.reduce((a, b) => a + b, 0);
        return exps.map(x => x / sum);
    }
    /**
     * Element-wise addition
     */
    add(a, b) {
        const len = Math.min(a.length, b.length);
        const result = new Array(len);
        for (let i = 0; i < len; i++) {
            result[i] = a[i] + b[i];
        }
        return result;
    }
    /**
     * Element-wise multiplication
     */
    mul(a, b) {
        const len = Math.min(a.length, b.length);
        const result = new Array(len);
        for (let i = 0; i < len; i++) {
            result[i] = a[i] * b[i];
        }
        return result;
    }
    /**
     * Scale vector by scalar
     */
    scale(a, scalar) {
        return a.map(x => x * scalar);
    }
    /**
     * Normalize vector to unit length
     */
    normalize(a) {
        const norm = Math.sqrt(a.reduce((sum, x) => sum + x * x, 0));
        return norm > 0 ? a.map(x => x / norm) : a;
    }
    /**
     * ReLU activation
     */
    relu(input) {
        return input.map(x => Math.max(0, x));
    }
    /**
     * GELU activation (approximate)
     */
    gelu(input) {
        return input.map(x => {
            return 0.5 * x * (1 + Math.tanh(Math.sqrt(2 / Math.PI) * (x + 0.044715 * x * x * x)));
        });
    }
    /**
     * Sigmoid activation
     */
    sigmoid(input) {
        return input.map(x => 1 / (1 + Math.exp(-x)));
    }
    /**
     * Layer normalization
     */
    layerNorm(input, eps = 1e-5) {
        const mean = input.reduce((a, b) => a + b, 0) / input.length;
        const variance = input.reduce((sum, x) => sum + (x - mean) ** 2, 0) / input.length;
        const std = Math.sqrt(variance + eps);
        return input.map(x => (x - mean) / std);
    }
    /**
     * Check if native SIMD is available
     */
    isNative() {
        return this.native !== null;
    }
    /**
     * Get available SIMD capabilities
     */
    capabilities() {
        if (!this.native) {
            return ['JavaScript (scalar)'];
        }
        // The native module will report actual capabilities
        const mod = (0, native_1.getNativeModule)();
        if (mod) {
            try {
                const engine = new mod.RuvLLMEngine();
                return engine.simdCapabilities();
            }
            catch {
                return ['Native (unknown)'];
            }
        }
        return ['JavaScript (scalar)'];
    }
}
exports.SimdOps = SimdOps;
//# sourceMappingURL=simd.js.map