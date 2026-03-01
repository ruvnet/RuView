/**
 * HybridSearch - Combined Vector + Keyword Search
 *
 * Implements Reciprocal Rank Fusion (RRF) to combine vector similarity
 * and BM25 keyword search for improved recall and precision.
 */
import { BM25Index } from './BM25Index.js';
import type { Embedder, VectorIndex } from '../memory/MemoryManager.js';
export interface HybridSearchConfig {
    vector: {
        enabled: boolean;
        weight: number;
    };
    keyword: {
        enabled: boolean;
        weight: number;
        k1?: number;
        b?: number;
    };
    fusion: {
        method: 'rrf' | 'linear' | 'weighted';
        k: number;
        candidateMultiplier: number;
    };
}
export interface HybridSearchResult {
    id: string;
    vectorScore: number;
    keywordScore: number;
    fusedScore: number;
    matchedTerms?: string[];
}
export interface HybridSearchOptions {
    topK?: number;
    threshold?: number;
    vectorOnly?: boolean;
    keywordOnly?: boolean;
}
export declare const DEFAULT_HYBRID_CONFIG: HybridSearchConfig;
export declare class HybridSearch {
    private readonly config;
    private vectorIndex;
    private embedder;
    private bm25Index;
    private initialized;
    constructor(config?: Partial<HybridSearchConfig>);
    /**
     * Initialize with vector index and embedder
     */
    initialize(vectorIndex: VectorIndex, embedder: Embedder): void;
    /**
     * Check if initialized
     */
    isInitialized(): boolean;
    /**
     * Add document to both indices
     */
    add(id: string, content: string, embedding?: Float32Array): Promise<void>;
    /**
     * Remove document from both indices
     */
    delete(id: string): boolean;
    /**
     * Hybrid search combining vector and keyword
     */
    search(query: string, options?: HybridSearchOptions): Promise<HybridSearchResult[]>;
    /**
     * Get statistics
     */
    getStats(): {
        config: HybridSearchConfig;
        bm25Stats: ReturnType<BM25Index['getStats']>;
        vectorIndexSize: number;
    };
    /**
     * Clear both indices
     */
    clear(): void;
    private vectorSearch;
    private keywordSearch;
    private fuseResults;
}
export declare function createHybridSearch(config?: Partial<HybridSearchConfig>): HybridSearch;
export default HybridSearch;
//# sourceMappingURL=HybridSearch.d.ts.map