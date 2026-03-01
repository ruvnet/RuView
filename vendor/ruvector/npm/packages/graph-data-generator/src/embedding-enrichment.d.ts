/**
 * Vector embedding enrichment for graph nodes and edges
 */
import { OpenRouterClient } from './openrouter-client.js';
import { GraphData, GraphNode, EmbeddingConfig } from './types.js';
export declare class EmbeddingEnrichment {
    private client;
    private config;
    constructor(client: OpenRouterClient, config?: Partial<EmbeddingConfig>);
    /**
     * Enrich graph data with vector embeddings
     */
    enrichGraphData(data: GraphData): Promise<GraphData>;
    /**
     * Enrich nodes with embeddings
     */
    private enrichNodes;
    /**
     * Enrich edges with embeddings
     */
    private enrichEdges;
    /**
     * Generate embedding for a node
     */
    private generateNodeEmbedding;
    /**
     * Generate embedding for an edge
     */
    private generateEdgeEmbedding;
    /**
     * Convert node to text for embedding
     */
    private nodeToText;
    /**
     * Convert edge to text for embedding
     */
    private edgeToText;
    /**
     * Generate embedding using OpenRouter or local model
     */
    private generateEmbedding;
    /**
     * Generate semantic embedding using chat model
     */
    private generateSemanticEmbedding;
    /**
     * Generate local embedding (placeholder)
     */
    private generateLocalEmbedding;
    /**
     * Generate random embedding (fallback)
     */
    private generateRandomEmbedding;
    /**
     * Calculate similarity between embeddings
     */
    calculateSimilarity(embedding1: number[], embedding2: number[], metric?: 'cosine' | 'euclidean' | 'dot'): number;
    /**
     * Calculate cosine similarity
     */
    private cosineSimilarity;
    /**
     * Calculate Euclidean distance
     */
    private euclideanDistance;
    /**
     * Calculate dot product
     */
    private dotProduct;
    /**
     * Find similar nodes using embeddings
     */
    findSimilarNodes(node: GraphNode, allNodes: GraphNode[], topK?: number, metric?: 'cosine' | 'euclidean' | 'dot'): Array<{
        node: GraphNode;
        similarity: number;
    }>;
}
/**
 * Create an embedding enrichment instance
 */
export declare function createEmbeddingEnrichment(client: OpenRouterClient, config?: Partial<EmbeddingConfig>): EmbeddingEnrichment;
//# sourceMappingURL=embedding-enrichment.d.ts.map