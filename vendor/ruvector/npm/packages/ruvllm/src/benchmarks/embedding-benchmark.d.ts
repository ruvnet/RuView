/**
 * Embedding Quality Benchmark for RuvLTRA Models
 *
 * Tests embedding quality for Claude Code use cases:
 * - Code similarity detection
 * - Task clustering
 * - Semantic search accuracy
 */
export interface EmbeddingPair {
    id: string;
    text1: string;
    text2: string;
    similarity: 'high' | 'medium' | 'low' | 'none';
    category: string;
}
export interface EmbeddingResult {
    pairId: string;
    expectedSimilarity: string;
    computedScore: number;
    correct: boolean;
    latencyMs: number;
}
export interface ClusterTestCase {
    id: string;
    items: string[];
    expectedCluster: string;
}
export interface EmbeddingBenchmarkResults {
    similarityAccuracy: number;
    similarityByCategory: Record<string, number>;
    avgSimilarityLatencyMs: number;
    clusterPurity: number;
    silhouetteScore: number;
    searchMRR: number;
    searchNDCG: number;
    similarityResults: EmbeddingResult[];
    totalPairs: number;
}
/**
 * Ground truth similarity pairs for testing
 * Tests whether embeddings correctly capture semantic similarity
 */
export declare const SIMILARITY_TEST_PAIRS: EmbeddingPair[];
/**
 * Search relevance test cases
 * Query + documents with relevance scores
 */
export interface SearchTestCase {
    id: string;
    query: string;
    documents: {
        text: string;
        relevance: number;
    }[];
}
export declare const SEARCH_TEST_CASES: SearchTestCase[];
/**
 * Cluster test cases - items that should cluster together
 */
export declare const CLUSTER_TEST_CASES: ClusterTestCase[];
/**
 * Check if computed similarity matches expected category
 */
export declare function isCorrectSimilarity(expected: 'high' | 'medium' | 'low' | 'none', computed: number): boolean;
/**
 * Calculate Mean Reciprocal Rank for search results
 */
export declare function calculateMRR(rankings: {
    relevant: boolean;
}[][]): number;
/**
 * Calculate NDCG for search results
 */
export declare function calculateNDCG(results: {
    relevance: number;
}[], idealOrder: {
    relevance: number;
}[]): number;
/**
 * Calculate silhouette score for clustering
 */
export declare function calculateSilhouette(embeddings: number[][], labels: number[]): number;
/**
 * Run the embedding benchmark
 */
export declare function runEmbeddingBenchmark(embedder: (text: string) => number[], similarityFn: (a: number[], b: number[]) => number): EmbeddingBenchmarkResults;
/**
 * Format embedding benchmark results for display
 */
export declare function formatEmbeddingResults(results: EmbeddingBenchmarkResults): string;
declare const _default: {
    SIMILARITY_TEST_PAIRS: EmbeddingPair[];
    SEARCH_TEST_CASES: SearchTestCase[];
    CLUSTER_TEST_CASES: ClusterTestCase[];
    runEmbeddingBenchmark: typeof runEmbeddingBenchmark;
    formatEmbeddingResults: typeof formatEmbeddingResults;
    isCorrectSimilarity: typeof isCorrectSimilarity;
    calculateMRR: typeof calculateMRR;
    calculateNDCG: typeof calculateNDCG;
};
export default _default;
//# sourceMappingURL=embedding-benchmark.d.ts.map