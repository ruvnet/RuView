/**
 * @ruvector/graph-data-generator - AI-powered synthetic graph data generation
 *
 * @packageDocumentation
 */

import 'dotenv/config';
import { OpenRouterClient, createOpenRouterClient } from './openrouter-client.js';
import {
  KnowledgeGraphGenerator,
  createKnowledgeGraphGenerator
} from './generators/knowledge-graph.js';
import {
  SocialNetworkGenerator,
  createSocialNetworkGenerator
} from './generators/social-network.js';
import {
  TemporalEventsGenerator,
  createTemporalEventsGenerator
} from './generators/temporal-events.js';
import {
  EntityRelationshipGenerator,
  createEntityRelationshipGenerator
} from './generators/entity-relationships.js';
import { CypherGenerator, createCypherGenerator } from './cypher-generator.js';
import { EmbeddingEnrichment, createEmbeddingEnrichment } from './embedding-enrichment.js';
import {
  OpenRouterConfig,
  KnowledgeGraphOptions,
  SocialNetworkOptions,
  TemporalEventOptions,
  EntityRelationshipOptions,
  GraphGenerationResult,
  GraphData,
  EmbeddingConfig
} from './types.js';

/**
 * Main GraphDataGenerator class
 */
export class GraphDataGenerator {
  private client: OpenRouterClient;
  private knowledgeGraphGen: KnowledgeGraphGenerator;
  private socialNetworkGen: SocialNetworkGenerator;
  private temporalEventsGen: TemporalEventsGenerator;
  private entityRelationshipGen: EntityRelationshipGenerator;
  private cypherGen: CypherGenerator;
  private embeddingEnrichment?: EmbeddingEnrichment;

  constructor(config: Partial<OpenRouterConfig> = {}) {
    this.client = createOpenRouterClient(config);
    this.knowledgeGraphGen = createKnowledgeGraphGenerator(this.client);
    this.socialNetworkGen = createSocialNetworkGenerator(this.client);
    this.temporalEventsGen = createTemporalEventsGenerator(this.client);
    this.entityRelationshipGen = createEntityRelationshipGenerator(this.client);
    this.cypherGen = createCypherGenerator();
  }

  /**
   * Generate a knowledge graph
   */
  async generateKnowledgeGraph(
    options: KnowledgeGraphOptions
  ): Promise<GraphGenerationResult<GraphData>> {
    const result = await this.knowledgeGraphGen.generate(options);

    // Add embeddings if requested
    if (options.includeEmbeddings) {
      result.data = await this.enrichWithEmbeddings(result.data, {
        dimensions: options.embeddingDimension
      });
    }

    // Generate Cypher statements
    result.cypher = this.cypherGen.generate(result.data);

    return result;
  }

  /**
   * Generate a social network
   */
  async generateSocialNetwork(
    options: SocialNetworkOptions
  ): Promise<GraphGenerationResult<GraphData>> {
    const result = await this.socialNetworkGen.generate(options);

    // Add embeddings if requested
    if (options.includeEmbeddings) {
      result.data = await this.enrichWithEmbeddings(result.data);
    }

    // Generate Cypher statements
    result.cypher = this.cypherGen.generate(result.data);

    return result;
  }

  /**
   * Generate temporal events
   */
  async generateTemporalEvents(
    options: TemporalEventOptions
  ): Promise<GraphGenerationResult<GraphData>> {
    const result = await this.temporalEventsGen.generate(options);

    // Add embeddings if requested
    if (options.includeEmbeddings) {
      result.data = await this.enrichWithEmbeddings(result.data);
    }

    // Generate Cypher statements
    result.cypher = this.cypherGen.generate(result.data);

    return result;
  }

  /**
   * Generate entity relationships
   */
  async generateEntityRelationships(
    options: EntityRelationshipOptions
  ): Promise<GraphGenerationResult<GraphData>> {
    const result = await this.entityRelationshipGen.generate(options);

    // Add embeddings if requested
    if (options.includeEmbeddings) {
      result.data = await this.enrichWithEmbeddings(result.data);
    }

    // Generate Cypher statements
    result.cypher = this.cypherGen.generate(result.data);

    return result;
  }

  /**
   * Enrich graph data with embeddings
   */
  async enrichWithEmbeddings(
    data: GraphData,
    config?: Partial<EmbeddingConfig>
  ): Promise<GraphData> {
    if (!this.embeddingEnrichment) {
      this.embeddingEnrichment = createEmbeddingEnrichment(this.client, config);
    }

    return this.embeddingEnrichment.enrichGraphData(data);
  }

  /**
   * Generate Cypher statements from graph data
   */
  generateCypher(data: GraphData, options?: {
    useConstraints?: boolean;
    useIndexes?: boolean;
    useMerge?: boolean;
  }): string {
    return this.cypherGen.generateSetupScript(data, options);
  }

  /**
   * Get OpenRouter client
   */
  getClient(): OpenRouterClient {
    return this.client;
  }

  /**
   * Get Cypher generator
   */
  getCypherGenerator(): CypherGenerator {
    return this.cypherGen;
  }

  /**
   * Get embedding enrichment
   */
  getEmbeddingEnrichment(): EmbeddingEnrichment | undefined {
    return this.embeddingEnrichment;
  }
}

/**
 * Create a new GraphDataGenerator instance
 */
export function createGraphDataGenerator(
  config?: Partial<OpenRouterConfig>
): GraphDataGenerator {
  return new GraphDataGenerator(config);
}

// Export all types and utilities
export * from './types.js';
export * from './openrouter-client.js';
export * from './generators/index.js';
export * from './cypher-generator.js';
export * from './embedding-enrichment.js';
export * from './schemas/index.js';

// Default export
export default GraphDataGenerator;
