/**
 * WASM Bridge - Actually functional WASM integration
 *
 * This module properly loads and uses the Rust-compiled WASM modules
 */
/**
 * Load the temporal neural solver WASM
 */
export declare function loadTemporalNeuralSolver(): Promise<any>;
/**
 * Load the graph reasoner WASM for PageRank
 */
export declare function loadGraphReasonerWasm(): Promise<any>;
/**
 * Load all available WASM modules
 */
export declare function initializeAllWasm(): Promise<{
    temporal: any;
    graph: any;
    hasWasm: boolean;
}>;
declare function multiplyMatrixVectorJS(matrix: Float64Array, vector: Float64Array, rows: number, cols: number): Float64Array;
declare function computePageRankJS(adjacency: Float64Array, n: number, damping: number, iterations: number): Float64Array;
export { multiplyMatrixVectorJS, computePageRankJS };
