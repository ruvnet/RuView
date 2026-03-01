"use strict";
/**
 * RuVector WASM Unified - Attention Engine
 *
 * Provides 14+ attention mechanisms including:
 * - 7 Neural attention mechanisms (scaled-dot, multi-head, hyperbolic, linear, flash, local-global, MoE, Mamba)
 * - 7 DAG attention mechanisms (topological, mincut-gated, hierarchical, spectral, flow, causal, sparse)
 */
Object.defineProperty(exports, "__esModule", { value: true });
exports.createAttentionEngine = createAttentionEngine;
exports.listAttentionMechanisms = listAttentionMechanisms;
exports.benchmarkAttention = benchmarkAttention;
// ============================================================================
// Factory and Utilities
// ============================================================================
/**
 * Create an attention engine instance
 * @param config Optional configuration
 * @returns Initialized attention engine
 */
function createAttentionEngine(config) {
    // Implementation delegated to WASM module
    return {
        scaledDot: (Q, K, V, mask) => {
            // WASM call: ruvector_attention_scaled_dot(Q, K, V, mask)
            const dk = Math.sqrt(Q.length / K.length);
            const scores = new Float32Array(Q.length);
            // Placeholder for WASM implementation
            return scores;
        },
        multiHead: (query, keys, values, config) => {
            // WASM call: ruvector_attention_multi_head(query, keys, values, config)
            return new Float32Array(query.length);
        },
        hyperbolic: (query, keys, values, curvature = -1) => {
            // WASM call: ruvector_attention_hyperbolic(query, keys, values, curvature)
            return new Float32Array(query.length);
        },
        linear: (query, keys, values, kernel = 'elu') => {
            // WASM call: ruvector_attention_linear(query, keys, values, kernel)
            return new Float32Array(query.length);
        },
        flash: (query, keys, values, blockSize = 256) => {
            // WASM call: ruvector_attention_flash(query, keys, values, blockSize)
            return new Float32Array(query.length);
        },
        localGlobal: (query, keys, values, windowSize, globalIndices = []) => {
            // WASM call: ruvector_attention_local_global(...)
            return new Float32Array(query.length);
        },
        moe: (query, keys, values, numExperts, topK, balanceLoss = true) => {
            // WASM call: ruvector_attention_moe(...)
            return {
                output: new Float32Array(query.length),
                routerLogits: new Float32Array(numExperts),
                expertUsage: new Float32Array(numExperts),
                loadBalanceLoss: 0,
            };
        },
        mamba: (input, state, config) => {
            // WASM call: ruvector_attention_mamba(input, state, config)
            return {
                output: new Float32Array(input.length),
                newState: new Float32Array(state.length),
                deltaTime: 0,
            };
        },
        dagTopological: (dag) => {
            // WASM call: ruvector_dag_topological(dag)
            return createEmptyScores('dag-topological');
        },
        dagMincutGated: (dag, gatePacket) => {
            // WASM call: ruvector_dag_mincut_gated(dag, gatePacket)
            return createEmptyScores('dag-mincut');
        },
        dagHierarchical: (dag, levels = 3) => {
            // WASM call: ruvector_dag_hierarchical(dag, levels)
            return createEmptyScores('dag-hierarchical');
        },
        dagSpectral: (dag, numEigenvectors = 16) => {
            // WASM call: ruvector_dag_spectral(dag, numEigenvectors)
            return createEmptyScores('dag-spectral');
        },
        dagFlow: (dag, sourceIds, sinkIds) => {
            // WASM call: ruvector_dag_flow(dag, sourceIds, sinkIds)
            return createEmptyScores('dag-flow');
        },
        dagCausal: (dag) => {
            // WASM call: ruvector_dag_causal(dag)
            return createEmptyScores('dag-causal');
        },
        dagSparse: (dag, sparsityRatio = 0.9) => {
            // WASM call: ruvector_dag_sparse(dag, sparsityRatio)
            return createEmptyScores('dag-sparse');
        },
    };
}
/** Create empty attention scores for placeholder returns */
function createEmptyScores(mechanism) {
    return {
        scores: new Float32Array(0),
        weights: new Float32Array(0),
        metadata: {
            mechanism,
            computeTimeMs: 0,
            memoryUsageBytes: 0,
        },
    };
}
/**
 * Get list of available attention mechanisms
 */
function listAttentionMechanisms() {
    return [
        'scaled-dot',
        'multi-head',
        'hyperbolic',
        'linear',
        'flash',
        'local-global',
        'moe',
        'mamba',
        'dag-topological',
        'dag-mincut',
        'dag-hierarchical',
        'dag-spectral',
        'dag-flow',
        'dag-causal',
        'dag-sparse',
    ];
}
/**
 * Benchmark attention mechanism performance
 * @param mechanism Mechanism to benchmark
 * @param inputSize Input tensor size
 * @param iterations Number of iterations
 * @returns Benchmark results
 */
async function benchmarkAttention(mechanism, inputSize, iterations = 100) {
    // Placeholder for benchmark implementation
    return {
        mechanism,
        avgTimeMs: 0,
        throughputOpsPerSec: 0,
        memoryPeakBytes: 0,
    };
}
//# sourceMappingURL=attention.js.map