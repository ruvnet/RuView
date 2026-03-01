/**
 * BM25Index - Full-Text Search with BM25 Scoring
 *
 * Implements the Okapi BM25 ranking algorithm for keyword-based search.
 * Used in hybrid search to complement vector similarity search.
 */
export interface BM25Config {
    k1: number;
    b: number;
}
export interface Document {
    id: string;
    content: string;
    tokens?: string[];
}
export interface BM25Result {
    id: string;
    score: number;
    matchedTerms: string[];
}
export declare class BM25Index {
    private readonly k1;
    private readonly b;
    private documents;
    private invertedIndex;
    private docFrequency;
    private docLengths;
    private avgDocLength;
    private readonly stopwords;
    constructor(config?: Partial<BM25Config>);
    /**
     * Add a document to the index
     */
    add(id: string, content: string): void;
    /**
     * Remove a document from the index
     */
    delete(id: string): boolean;
    /**
     * Search the index with BM25 scoring
     */
    search(query: string, topK?: number): BM25Result[];
    /**
     * Get document by ID
     */
    get(id: string): Document | undefined;
    /**
     * Check if document exists
     */
    has(id: string): boolean;
    /**
     * Get index size
     */
    size(): number;
    /**
     * Clear all documents
     */
    clear(): void;
    /**
     * Get index statistics
     */
    getStats(): {
        documentCount: number;
        uniqueTerms: number;
        avgDocLength: number;
        k1: number;
        b: number;
    };
    /**
     * Tokenize text into normalized terms
     */
    private tokenize;
    /**
     * Simple stemming (basic suffix removal)
     */
    private stem;
    /**
     * Count term frequency in tokens
     */
    private termFrequency;
    /**
     * Update average document length
     */
    private updateAvgDocLength;
}
export declare function createBM25Index(config?: Partial<BM25Config>): BM25Index;
export default BM25Index;
//# sourceMappingURL=BM25Index.d.ts.map