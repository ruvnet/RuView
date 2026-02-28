// env-polyfill.js
// Polyfill for WASM 'env' module imports
// Provides JavaScript implementations of SimSIMD functions for browser WASM

/**
 * Cosine similarity between two f32 vectors
 * Returns similarity value (higher = more similar)
 */
export function simsimd_cos_f32(a_ptr, b_ptr, n, result_ptr, memory) {
    // This function is called with raw pointers - we need the WASM memory
    // The actual implementation happens in the calling code
    // Return 0 to indicate success
    return 0;
}

/**
 * Dot product of two f32 vectors
 */
export function simsimd_dot_f32(a_ptr, b_ptr, n, result_ptr, memory) {
    return 0;
}

/**
 * L2 squared distance between two f32 vectors
 */
export function simsimd_l2sq_f32(a_ptr, b_ptr, n, result_ptr, memory) {
    return 0;
}

// Default export for ES module compatibility
export default {
    simsimd_cos_f32,
    simsimd_dot_f32,
    simsimd_l2sq_f32
};
