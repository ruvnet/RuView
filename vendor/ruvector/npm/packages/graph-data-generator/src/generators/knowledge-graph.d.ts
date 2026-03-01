/**
 * Knowledge graph generator using OpenRouter/Kimi K2
 */
import { OpenRouterClient } from '../openrouter-client.js';
import { KnowledgeGraphOptions, GraphData, GraphGenerationResult, KnowledgeTriple } from '../types.js';
export declare class KnowledgeGraphGenerator {
    private client;
    constructor(client: OpenRouterClient);
    /**
     * Generate a knowledge graph
     */
    generate(options: KnowledgeGraphOptions): Promise<GraphGenerationResult<GraphData>>;
    /**
     * Generate entities for the knowledge graph
     */
    private generateEntities;
    /**
     * Generate relationships between entities
     */
    private generateRelationships;
    /**
     * Generate knowledge triples (subject-predicate-object)
     */
    generateTriples(domain: string, count: number): Promise<KnowledgeTriple[]>;
}
/**
 * Create a knowledge graph generator
 */
export declare function createKnowledgeGraphGenerator(client: OpenRouterClient): KnowledgeGraphGenerator;
//# sourceMappingURL=knowledge-graph.d.ts.map