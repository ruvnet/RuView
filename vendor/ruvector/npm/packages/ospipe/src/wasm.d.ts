/**
 * WASM bindings for OsPipe - use in browser-based pipes.
 *
 * This module provides a thin wrapper around the @ruvector/ospipe-wasm package,
 * exposing vector search, embedding, deduplication, and safety checking
 * capabilities that run entirely client-side via WebAssembly.
 *
 * @packageDocumentation
 */
/** A single search result from the WASM vector index. */
export interface WasmSearchResult {
    /** Unique identifier for the indexed entry */
    id: string;
    /** Similarity score (higher is more similar) */
    score: number;
    /** JSON-encoded metadata string */
    metadata: string;
}
/** Configuration options for WASM initialization. */
export interface OsPipeWasmOptions {
    /** Embedding vector dimension (default: 384) */
    dimension?: number;
}
/** The initialized WASM instance interface. */
export interface OsPipeWasmInstance {
    /**
     * Insert a vector into the index.
     *
     * @param id - Unique identifier for the entry
     * @param embedding - Float32Array embedding vector
     * @param metadata - JSON-encoded metadata string
     * @param timestamp - Unix timestamp in milliseconds (default: Date.now())
     */
    insert(id: string, embedding: Float32Array, metadata: string, timestamp?: number): void;
    /**
     * Search for the k nearest neighbors to the query embedding.
     *
     * @param queryEmbedding - Float32Array query vector
     * @param k - Number of results to return (default: 10)
     * @returns Array of search results ranked by similarity
     */
    search(queryEmbedding: Float32Array, k?: number): WasmSearchResult[];
    /**
     * Search with a time range filter applied before ranking.
     *
     * @param queryEmbedding - Float32Array query vector
     * @param k - Number of results to return
     * @param startTime - Start of time range (Unix ms)
     * @param endTime - End of time range (Unix ms)
     * @returns Array of filtered search results
     */
    searchFiltered(queryEmbedding: Float32Array, k: number, startTime: number, endTime: number): WasmSearchResult[];
    /**
     * Check if an embedding is a near-duplicate of an existing entry.
     *
     * @param embedding - Float32Array embedding to check
     * @param threshold - Similarity threshold 0-1 (default: 0.95)
     * @returns True if a duplicate is found above the threshold
     */
    isDuplicate(embedding: Float32Array, threshold?: number): boolean;
    /**
     * Generate an embedding vector from text using the built-in ONNX model.
     *
     * @param text - Input text to embed
     * @returns Float32Array embedding vector
     */
    embedText(text: string): Float32Array;
    /**
     * Run a safety check on content, returning the recommended action.
     *
     * @param content - Content string to check
     * @returns "allow", "redact", or "deny"
     */
    safetyCheck(content: string): "allow" | "redact" | "deny";
    /**
     * Route a query to the optimal query type.
     *
     * @param query - Natural language query string
     * @returns Recommended query route type
     */
    routeQuery(query: string): string;
    /** Number of entries currently in the index. */
    readonly size: number;
    /**
     * Get index statistics as a JSON string.
     *
     * @returns JSON-encoded statistics object
     */
    stats(): string;
}
/**
 * Load and initialize the OsPipe WASM module.
 *
 * This function dynamically imports the @ruvector/ospipe-wasm package,
 * initializes the WebAssembly module, and returns a typed wrapper
 * around the raw WASM bindings.
 *
 * @param options - WASM initialization options
 * @returns Initialized WASM instance with typed methods
 * @throws {Error} If the WASM module fails to load or initialize
 *
 * @example
 * ```typescript
 * import { initOsPipeWasm } from "@ruvector/ospipe/wasm";
 *
 * const wasm = await initOsPipeWasm({ dimension: 384 });
 *
 * // Embed and insert
 * const embedding = wasm.embedText("hello world");
 * wasm.insert("doc-1", embedding, JSON.stringify({ app: "test" }));
 *
 * // Search
 * const query = wasm.embedText("greetings");
 * const results = wasm.search(query, 5);
 * ```
 */
export declare function initOsPipeWasm(options?: OsPipeWasmOptions): Promise<OsPipeWasmInstance>;
//# sourceMappingURL=wasm.d.ts.map