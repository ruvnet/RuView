/**
 * Knowledge graph generator using OpenRouter/Kimi K2
 */

import { OpenRouterClient } from '../openrouter-client.js';
import {
  KnowledgeGraphOptions,
  GraphData,
  GraphNode,
  GraphEdge,
  GraphGenerationResult,
  KnowledgeTriple
} from '../types.js';

interface EntityData {
  id: string;
  name: string;
  type: string;
  properties: Record<string, unknown>;
}

interface RelationshipData {
  source: string;
  target: string;
  type: string;
  properties?: Record<string, unknown>;
}

const DEFAULT_ENTITY_TYPES = [
  'Person',
  'Organization',
  'Location',
  'Event',
  'Concept',
  'Technology',
  'Product'
];

const DEFAULT_RELATIONSHIP_TYPES = [
  'WORKS_FOR',
  'LOCATED_IN',
  'CREATED_BY',
  'PART_OF',
  'RELATED_TO',
  'INFLUENCES',
  'DEPENDS_ON'
];

export class KnowledgeGraphGenerator {
  constructor(private client: OpenRouterClient) {}

  /**
   * Generate a knowledge graph
   */
  async generate(options: KnowledgeGraphOptions): Promise<GraphGenerationResult<GraphData>> {
    const startTime = Date.now();

    // Generate entities first
    const entities = await this.generateEntities(options);

    // Generate relationships between entities
    const relationships = await this.generateRelationships(entities, options);

    // Convert to graph structure
    const nodes: GraphNode[] = entities.map((entity, idx) => ({
      id: entity.id || `entity_${idx}`,
      labels: [entity.type || 'Entity'],
      properties: {
        name: entity.name,
        ...entity.properties
      }
    }));

    const edges: GraphEdge[] = relationships.map((rel, idx) => ({
      id: `rel_${idx}`,
      type: rel.type,
      source: rel.source,
      target: rel.target,
      properties: rel.properties || {}
    }));

    const data: GraphData = {
      nodes,
      edges,
      metadata: {
        domain: options.domain,
        generated_at: new Date(),
        total_nodes: nodes.length,
        total_edges: edges.length
      }
    };

    return {
      data,
      metadata: {
        generated_at: new Date(),
        model: this.client.getConfig().model || 'moonshot/kimi-k2-instruct',
        duration: Date.now() - startTime
      }
    };
  }

  /**
   * Generate entities for the knowledge graph
   */
  private async generateEntities(options: KnowledgeGraphOptions): Promise<Array<{
    id: string;
    name: string;
    type: string;
    properties: Record<string, unknown>;
  }>> {
    const entityTypes = options.entityTypes || DEFAULT_ENTITY_TYPES;

    const systemPrompt = `You are an expert knowledge graph architect. Generate realistic entities for a knowledge graph about ${options.domain}.`;

    const userPrompt = `Generate ${options.entities} diverse entities for a knowledge graph about ${options.domain}.

Entity types to include: ${entityTypes.join(', ')}

For each entity, provide:
- id: unique identifier (use snake_case)
- name: entity name
- type: one of the specified entity types
- properties: relevant properties (at least 2-3 properties per entity)

Return a JSON array of entities. Make them realistic and relevant to ${options.domain}.

Example format:
\`\`\`json
[
  {
    "id": "john_doe",
    "name": "John Doe",
    "type": "Person",
    "properties": {
      "role": "Software Engineer",
      "expertise": "AI/ML",
      "years_experience": 5
    }
  }
]
\`\`\``;

    const entities = await this.client.generateStructured<EntityData[]>(systemPrompt, userPrompt, {
      temperature: 0.8,
      maxTokens: Math.min(8000, options.entities * 100)
    });

    return entities;
  }

  /**
   * Generate relationships between entities
   */
  private async generateRelationships(
    entities: EntityData[],
    options: KnowledgeGraphOptions
  ): Promise<RelationshipData[]> {
    const relationshipTypes = options.relationshipTypes || DEFAULT_RELATIONSHIP_TYPES;

    const systemPrompt = `You are an expert at creating meaningful relationships in knowledge graphs. Create realistic relationships that make sense for ${options.domain}.`;

    const entityList = entities.slice(0, 50).map(e => `- ${e.id}: ${e.name} (${e.type})`).join('\n');

    const userPrompt = `Given these entities from a ${options.domain} knowledge graph:

${entityList}

Generate ${options.relationships} meaningful relationships between them.

Relationship types to use: ${relationshipTypes.join(', ')}

For each relationship, provide:
- source: source entity id
- target: target entity id
- type: relationship type (use one of the specified types)
- properties: optional properties describing the relationship

Return a JSON array of relationships. Make them logical and realistic.

Example format:
\`\`\`json
[
  {
    "source": "john_doe",
    "target": "acme_corp",
    "type": "WORKS_FOR",
    "properties": {
      "since": "2020",
      "position": "Senior Engineer"
    }
  }
]
\`\`\``;

    const relationships = await this.client.generateStructured<RelationshipData[]>(systemPrompt, userPrompt, {
      temperature: 0.7,
      maxTokens: Math.min(8000, options.relationships * 80)
    });

    return relationships;
  }

  /**
   * Generate knowledge triples (subject-predicate-object)
   */
  async generateTriples(
    domain: string,
    count: number
  ): Promise<KnowledgeTriple[]> {
    const systemPrompt = `You are an expert at extracting knowledge triples from domains. Generate meaningful subject-predicate-object triples about ${domain}.`;

    const userPrompt = `Generate ${count} knowledge triples about ${domain}.

Each triple should have:
- subject: the entity or concept
- predicate: the relationship or property
- object: the related entity, value, or concept
- confidence: confidence score (0-1)

Return a JSON array of triples.

Example format:
\`\`\`json
[
  {
    "subject": "Einstein",
    "predicate": "developed",
    "object": "Theory of Relativity",
    "confidence": 1.0
  }
]
\`\`\``;

    return this.client.generateStructured<KnowledgeTriple[]>(
      systemPrompt,
      userPrompt,
      { temperature: 0.7, maxTokens: count * 100 }
    );
  }
}

/**
 * Create a knowledge graph generator
 */
export function createKnowledgeGraphGenerator(client: OpenRouterClient): KnowledgeGraphGenerator {
  return new KnowledgeGraphGenerator(client);
}
