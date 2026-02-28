/**
 * Entity relationship generator for domain-specific graphs
 */
import { OpenRouterClient } from '../openrouter-client.js';
import { EntityRelationshipOptions, GraphData, GraphGenerationResult } from '../types.js';
export declare class EntityRelationshipGenerator {
    private client;
    constructor(client: OpenRouterClient);
    /**
     * Generate entity-relationship graph
     */
    generate(options: EntityRelationshipOptions): Promise<GraphGenerationResult<GraphData>>;
    /**
     * Generate domain-specific entities
     */
    private generateEntities;
    /**
     * Generate relationships between entities
     */
    private generateRelationships;
    /**
     * Generate schema-aware entities and relationships
     */
    generateWithSchema(schema: {
        entities: Record<string, {
            properties: Record<string, string>;
            relationships?: string[];
        }>;
        relationships: Record<string, {
            from: string;
            to: string;
            properties?: Record<string, string>;
        }>;
    }, count: number): Promise<GraphData>;
    /**
     * Analyze entity-relationship patterns
     */
    analyzeERPatterns(data: GraphData): Promise<{
        entityTypeDistribution: Record<string, number>;
        relationshipTypeDistribution: Record<string, number>;
        avgRelationshipsPerEntity: number;
        densityScore: number;
    }>;
}
/**
 * Create an entity relationship generator
 */
export declare function createEntityRelationshipGenerator(client: OpenRouterClient): EntityRelationshipGenerator;
//# sourceMappingURL=entity-relationships.d.ts.map