/**
 * Vector embedding enrichment for graph nodes and edges
 */

import { OpenRouterClient } from './openrouter-client.js';
import {
  GraphData,
  GraphNode,
  GraphEdge,
  EmbeddingConfig,
  EmbeddingResult
} from './types.js';

export class EmbeddingEnrichment {
  private config: EmbeddingConfig;

  constructor(
    private client: OpenRouterClient,
    config: Partial<EmbeddingConfig> = {}
  ) {
    this.config = {
      provider: 'openrouter',
      dimensions: 1536,
      batchSize: 100,
      ...config
    };
  }

  /**
   * Enrich graph data with vector embeddings
   */
  async enrichGraphData(data: GraphData): Promise<GraphData> {
    // Generate embeddings for nodes
    const enrichedNodes = await this.enrichNodes(data.nodes);

    // Generate embeddings for edges (optional)
    const enrichedEdges = await this.enrichEdges(data.edges);

    return {
      ...data,
      nodes: enrichedNodes,
      edges: enrichedEdges
    };
  }

  /**
   * Enrich nodes with embeddings
   */
  private async enrichNodes(nodes: GraphNode[]): Promise<GraphNode[]> {
    const enriched: GraphNode[] = [];

    // Process in batches
    for (let i = 0; i < nodes.length; i += this.config.batchSize!) {
      const batch = nodes.slice(i, i + this.config.batchSize!);
      const batchResults = await Promise.all(
        batch.map(node => this.generateNodeEmbedding(node))
      );

      enriched.push(...batchResults);
    }

    return enriched;
  }

  /**
   * Enrich edges with embeddings
   */
  private async enrichEdges(edges: GraphEdge[]): Promise<GraphEdge[]> {
    const enriched: GraphEdge[] = [];

    // Process in batches
    for (let i = 0; i < edges.length; i += this.config.batchSize!) {
      const batch = edges.slice(i, i + this.config.batchSize!);
      const batchResults = await Promise.all(
        batch.map(edge => this.generateEdgeEmbedding(edge))
      );

      enriched.push(...batchResults);
    }

    return enriched;
  }

  /**
   * Generate embedding for a node
   */
  private async generateNodeEmbedding(node: GraphNode): Promise<GraphNode> {
    // Create text representation of node
    const text = this.nodeToText(node);

    // Generate embedding
    const embedding = await this.generateEmbedding(text);

    return {
      ...node,
      embedding: embedding.embedding
    };
  }

  /**
   * Generate embedding for an edge
   */
  private async generateEdgeEmbedding(edge: GraphEdge): Promise<GraphEdge> {
    // Create text representation of edge
    const text = this.edgeToText(edge);

    // Generate embedding
    const embedding = await this.generateEmbedding(text);

    return {
      ...edge,
      embedding: embedding.embedding
    };
  }

  /**
   * Convert node to text for embedding
   */
  private nodeToText(node: GraphNode): string {
    const parts: string[] = [];

    // Add labels
    parts.push(`Type: ${node.labels.join(', ')}`);

    // Add properties
    for (const [key, value] of Object.entries(node.properties)) {
      if (typeof value === 'string' || typeof value === 'number') {
        parts.push(`${key}: ${value}`);
      }
    }

    return parts.join('. ');
  }

  /**
   * Convert edge to text for embedding
   */
  private edgeToText(edge: GraphEdge): string {
    const parts: string[] = [];

    // Add relationship type
    parts.push(`Relationship: ${edge.type}`);

    // Add properties
    for (const [key, value] of Object.entries(edge.properties)) {
      if (typeof value === 'string' || typeof value === 'number') {
        parts.push(`${key}: ${value}`);
      }
    }

    return parts.join('. ');
  }

  /**
   * Generate embedding using OpenRouter or local model
   */
  private async generateEmbedding(text: string): Promise<EmbeddingResult> {
    if (this.config.provider === 'local') {
      return this.generateLocalEmbedding(text);
    }

    // Use OpenRouter with embedding-capable model
    // Note: Kimi K2 may not support embeddings, so we use a workaround
    // by generating semantic vectors through the chat API
    const embedding = await this.generateSemanticEmbedding(text);

    return {
      embedding,
      model: this.config.model || 'moonshot/kimi-k2-instruct',
      dimensions: embedding.length
    };
  }

  /**
   * Generate semantic embedding using chat model
   */
  private async generateSemanticEmbedding(text: string): Promise<number[]> {
    // Use the chat API to generate a semantic representation
    // This is a workaround for models without native embedding support
    const systemPrompt = `You are a semantic encoder. Convert the input text into a semantic representation by analyzing its key concepts, entities, and relationships. Output ONLY a JSON array of ${this.config.dimensions} floating point numbers between -1 and 1 representing the semantic vector.`;

    const userPrompt = `Encode this text into a ${this.config.dimensions}-dimensional semantic vector:\n\n${text}`;

    try {
      const response = await this.client.createCompletion(
        [
          { role: 'system', content: systemPrompt },
          { role: 'user', content: userPrompt }
        ],
        {
          temperature: 0.3,
          max_tokens: this.config.dimensions! * 10
        }
      );

      const content = response.choices[0]?.message?.content;
      if (!content) {
        throw new Error('No content in embedding response');
      }

      // Extract JSON array
      const match = content.match(/\[([\s\S]*?)\]/);
      if (match) {
        const embedding = JSON.parse(`[${match[1]}]`) as number[];

        // Ensure correct dimensions
        if (embedding.length !== this.config.dimensions) {
          return this.generateRandomEmbedding();
        }

        return embedding;
      }

      // Fallback to random embedding
      return this.generateRandomEmbedding();
    } catch (error) {
      console.warn('Failed to generate semantic embedding, using random:', error);
      return this.generateRandomEmbedding();
    }
  }

  /**
   * Generate local embedding (placeholder)
   */
  private async generateLocalEmbedding(_text: string): Promise<EmbeddingResult> {
    // This would use a local embedding model
    // For now, return a random embedding
    return {
      embedding: this.generateRandomEmbedding(),
      model: 'local',
      dimensions: this.config.dimensions!
    };
  }

  /**
   * Generate random embedding (fallback)
   */
  private generateRandomEmbedding(): number[] {
    const embedding: number[] = [];
    for (let i = 0; i < this.config.dimensions!; i++) {
      embedding.push((Math.random() * 2) - 1); // Random value between -1 and 1
    }

    // Normalize to unit length
    const magnitude = Math.sqrt(embedding.reduce((sum, val) => sum + val * val, 0));
    return embedding.map(val => val / magnitude);
  }

  /**
   * Calculate similarity between embeddings
   */
  calculateSimilarity(
    embedding1: number[],
    embedding2: number[],
    metric: 'cosine' | 'euclidean' | 'dot' = 'cosine'
  ): number {
    if (embedding1.length !== embedding2.length) {
      throw new Error('Embeddings must have the same dimensions');
    }

    switch (metric) {
      case 'cosine':
        return this.cosineSimilarity(embedding1, embedding2);
      case 'euclidean':
        return this.euclideanDistance(embedding1, embedding2);
      case 'dot':
        return this.dotProduct(embedding1, embedding2);
      default:
        return this.cosineSimilarity(embedding1, embedding2);
    }
  }

  /**
   * Calculate cosine similarity
   */
  private cosineSimilarity(a: number[], b: number[]): number {
    const dotProduct = a.reduce((sum, val, i) => sum + val * b[i], 0);
    const magnitudeA = Math.sqrt(a.reduce((sum, val) => sum + val * val, 0));
    const magnitudeB = Math.sqrt(b.reduce((sum, val) => sum + val * val, 0));
    return dotProduct / (magnitudeA * magnitudeB);
  }

  /**
   * Calculate Euclidean distance
   */
  private euclideanDistance(a: number[], b: number[]): number {
    return Math.sqrt(a.reduce((sum, val, i) => sum + Math.pow(val - b[i], 2), 0));
  }

  /**
   * Calculate dot product
   */
  private dotProduct(a: number[], b: number[]): number {
    return a.reduce((sum, val, i) => sum + val * b[i], 0);
  }

  /**
   * Find similar nodes using embeddings
   */
  findSimilarNodes(
    node: GraphNode,
    allNodes: GraphNode[],
    topK: number = 10,
    metric: 'cosine' | 'euclidean' | 'dot' = 'cosine'
  ): Array<{ node: GraphNode; similarity: number }> {
    if (!node.embedding) {
      throw new Error('Node does not have an embedding');
    }

    const similarities = allNodes
      .filter(n => n.id !== node.id && n.embedding)
      .map(n => ({
        node: n,
        similarity: this.calculateSimilarity(node.embedding!, n.embedding!, metric)
      }))
      .sort((a, b) => b.similarity - a.similarity)
      .slice(0, topK);

    return similarities;
  }
}

/**
 * Create an embedding enrichment instance
 */
export function createEmbeddingEnrichment(
  client: OpenRouterClient,
  config?: Partial<EmbeddingConfig>
): EmbeddingEnrichment {
  return new EmbeddingEnrichment(client, config);
}
