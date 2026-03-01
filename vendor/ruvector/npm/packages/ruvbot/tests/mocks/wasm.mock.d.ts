/**
 * WASM Mock Module
 *
 * Mock implementations for RuVector WASM bindings
 * Used to test code that depends on WASM modules without loading actual binaries
 */
export interface WasmVectorIndex {
    add(id: string, vector: Float32Array): void;
    search(query: Float32Array, topK: number): SearchResult[];
    delete(id: string): boolean;
    size(): number;
    clear(): void;
}
export interface SearchResult {
    id: string;
    score: number;
    distance: number;
}
export interface WasmEmbedder {
    embed(text: string): Float32Array;
    embedBatch(texts: string[]): Float32Array[];
    dimension(): number;
}
export interface WasmRouter {
    route(input: string, context?: Record<string, unknown>): RouteResult;
    addRoute(pattern: string, handler: string): void;
    removeRoute(pattern: string): boolean;
}
export interface RouteResult {
    handler: string;
    confidence: number;
    metadata: Record<string, unknown>;
}
/**
 * Mock WASM Vector Index
 */
export declare class MockWasmVectorIndex implements WasmVectorIndex {
    private vectors;
    private dimension;
    constructor(dimension?: number);
    add(id: string, vector: Float32Array): void;
    search(query: Float32Array, topK: number): SearchResult[];
    delete(id: string): boolean;
    size(): number;
    clear(): void;
    private cosineSimilarity;
}
/**
 * Mock WASM Embedder
 */
export declare class MockWasmEmbedder implements WasmEmbedder {
    private dim;
    private cache;
    constructor(dimension?: number);
    embed(text: string): Float32Array;
    embedBatch(texts: string[]): Float32Array[];
    dimension(): number;
    private hashCode;
}
/**
 * Mock WASM Router
 */
export declare class MockWasmRouter implements WasmRouter {
    private routes;
    route(input: string, context?: Record<string, unknown>): RouteResult;
    addRoute(pattern: string, handler: string): void;
    removeRoute(pattern: string): boolean;
}
/**
 * Mock WASM Module Loader
 */
export declare const mockWasmLoader: {
    loadVectorIndex: import("vitest").Mock<[dimension?: number | undefined], Promise<WasmVectorIndex>>;
    loadEmbedder: import("vitest").Mock<[dimension?: number | undefined], Promise<WasmEmbedder>>;
    loadRouter: import("vitest").Mock<[], Promise<WasmRouter>>;
    isWasmSupported: import("vitest").Mock<[], boolean>;
    getWasmMemory: import("vitest").Mock<[], {
        used: number;
        total: number;
    }>;
};
/**
 * Create mock WASM bindings for RuVector
 */
export declare function createMockRuVectorBindings(): {
    vectorIndex: MockWasmVectorIndex;
    embedder: MockWasmEmbedder;
    router: MockWasmRouter;
    search(query: string, topK?: number): Promise<SearchResult[]>;
    index(id: string, text: string): Promise<void>;
    batchIndex(items: Array<{
        id: string;
        text: string;
    }>): Promise<void>;
};
/**
 * Reset all WASM mocks
 */
export declare function resetWasmMocks(): void;
declare const _default: {
    MockWasmVectorIndex: typeof MockWasmVectorIndex;
    MockWasmEmbedder: typeof MockWasmEmbedder;
    MockWasmRouter: typeof MockWasmRouter;
    mockWasmLoader: {
        loadVectorIndex: import("vitest").Mock<[dimension?: number | undefined], Promise<WasmVectorIndex>>;
        loadEmbedder: import("vitest").Mock<[dimension?: number | undefined], Promise<WasmEmbedder>>;
        loadRouter: import("vitest").Mock<[], Promise<WasmRouter>>;
        isWasmSupported: import("vitest").Mock<[], boolean>;
        getWasmMemory: import("vitest").Mock<[], {
            used: number;
            total: number;
        }>;
    };
    createMockRuVectorBindings: typeof createMockRuVectorBindings;
    resetWasmMocks: typeof resetWasmMocks;
};
export default _default;
//# sourceMappingURL=wasm.mock.d.ts.map