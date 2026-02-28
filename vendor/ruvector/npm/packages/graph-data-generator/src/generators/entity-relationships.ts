/**
 * Entity relationship generator for domain-specific graphs
 */

import { OpenRouterClient } from '../openrouter-client.js';
import {
  EntityRelationshipOptions,
  GraphData,
  GraphNode,
  GraphEdge,
  GraphGenerationResult
} from '../types.js';

interface EntityData {
  id: string;
  labels: string[];
  properties: Record<string, unknown>;
}

interface RelationshipData {
  source: string;
  target: string;
  type: string;
  properties?: Record<string, unknown>;
}

export class EntityRelationshipGenerator {
  constructor(private client: OpenRouterClient) {}

  /**
   * Generate entity-relationship graph
   */
  async generate(options: EntityRelationshipOptions): Promise<GraphGenerationResult<GraphData>> {
    const startTime = Date.now();

    // Generate entities
    const entities = await this.generateEntities(options);

    // Generate relationships
    const relationships = await this.generateRelationships(entities, options);

    // Convert to graph structure
    const nodes: GraphNode[] = entities.map(entity => ({
      id: entity.id,
      labels: entity.labels || ['Entity'],
      properties: entity.properties
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
   * Generate domain-specific entities
   */
  private async generateEntities(options: EntityRelationshipOptions): Promise<EntityData[]> {
    const systemPrompt = `You are an expert in ${options.domain} domain modeling. Generate realistic entities following best practices for ${options.domain} data models.`;

    const schemaInfo = options.entitySchema
      ? `\n\nEntity schema to follow:\n${JSON.stringify(options.entitySchema, null, 2)}`
      : '';

    const userPrompt = `Generate ${options.entityCount} diverse entities for a ${options.domain} domain model.${schemaInfo}

Each entity should have:
- id: unique identifier (use snake_case)
- labels: array of entity type labels (e.g., ["Product", "Digital"])
- properties: object with entity properties (at least 3-5 meaningful properties)

Make entities realistic and relevant to ${options.domain}. Include variety in types and attributes.

Return a JSON array of entities.

Example format:
\`\`\`json
[
  {
    "id": "product_laptop_001",
    "labels": ["Product", "Electronics", "Computer"],
    "properties": {
      "name": "UltraBook Pro 15",
      "category": "Laptops",
      "price": 1299.99,
      "brand": "TechCorp",
      "release_date": "2024-01-15",
      "stock": 45,
      "rating": 4.7
    }
  }
]
\`\`\``;

    return this.client.generateStructured<EntityData[]>(systemPrompt, userPrompt, {
      temperature: 0.8,
      maxTokens: Math.min(8000, options.entityCount * 150)
    });
  }

  /**
   * Generate relationships between entities
   */
  private async generateRelationships(
    entities: EntityData[],
    options: EntityRelationshipOptions
  ): Promise<RelationshipData[]> {
    // Calculate target relationship count based on density
    const maxPossibleRelationships = entities.length * (entities.length - 1);
    const targetRelationships = Math.floor(maxPossibleRelationships * options.relationshipDensity);

    const relationshipTypes = options.relationshipTypes || [
      'RELATES_TO',
      'PART_OF',
      'DEPENDS_ON',
      'SIMILAR_TO',
      'CONTAINS'
    ];

    const systemPrompt = `You are an expert in ${options.domain} domain modeling. Create meaningful, realistic relationships between entities.`;

    const entityList = entities.slice(0, 100).map(e =>
      `- ${e.id} (${e.labels.join(', ')}): ${JSON.stringify(e.properties).substring(0, 100)}`
    ).join('\n');

    const userPrompt = `Given these entities from a ${options.domain} domain:

${entityList}

Generate ${targetRelationships} meaningful relationships between them.

Relationship types to use: ${relationshipTypes.join(', ')}

Each relationship should have:
- source: source entity id
- target: target entity id
- type: relationship type (use UPPER_SNAKE_CASE)
- properties: optional properties describing the relationship

Make relationships logical and realistic for ${options.domain}. Avoid creating too many relationships from/to the same entity.

Return a JSON array of relationships.

Example format:
\`\`\`json
[
  {
    "source": "product_laptop_001",
    "target": "category_electronics",
    "type": "BELONGS_TO",
    "properties": {
      "primary": true,
      "added_date": "2024-01-15"
    }
  }
]
\`\`\``;

    return this.client.generateStructured<RelationshipData[]>(systemPrompt, userPrompt, {
      temperature: 0.7,
      maxTokens: Math.min(8000, targetRelationships * 80)
    });
  }

  /**
   * Generate schema-aware entities and relationships
   */
  async generateWithSchema(
    schema: {
      entities: Record<string, {
        properties: Record<string, string>;
        relationships?: string[];
      }>;
      relationships: Record<string, {
        from: string;
        to: string;
        properties?: Record<string, string>;
      }>;
    },
    count: number
  ): Promise<GraphData> {
    const systemPrompt = 'You are an expert at generating synthetic data that conforms to strict schemas.';

    const userPrompt = `Generate ${count} instances of entities and relationships following this exact schema:

${JSON.stringify(schema, null, 2)}

Return a JSON object with:
- nodes: array of entities matching the entity types in the schema
- edges: array of relationships matching the relationship types in the schema

Ensure all properties match their specified types and all relationships connect valid entity types.

Example format:
\`\`\`json
{
  "nodes": [...],
  "edges": [...]
}
\`\`\``;

    return this.client.generateStructured<GraphData>(
      systemPrompt,
      userPrompt,
      {
        temperature: 0.7,
        maxTokens: Math.min(8000, count * 200)
      }
    );
  }

  /**
   * Analyze entity-relationship patterns
   */
  async analyzeERPatterns(data: GraphData): Promise<{
    entityTypeDistribution: Record<string, number>;
    relationshipTypeDistribution: Record<string, number>;
    avgRelationshipsPerEntity: number;
    densityScore: number;
  }> {
    const entityTypeDistribution: Record<string, number> = {};
    const relationshipTypeDistribution: Record<string, number> = {};
    const entityDegrees = new Map<string, number>();

    // Count entity types
    for (const node of data.nodes) {
      for (const label of node.labels) {
        entityTypeDistribution[label] = (entityTypeDistribution[label] || 0) + 1;
      }
    }

    // Count relationship types and degrees
    for (const edge of data.edges) {
      relationshipTypeDistribution[edge.type] = (relationshipTypeDistribution[edge.type] || 0) + 1;
      entityDegrees.set(edge.source, (entityDegrees.get(edge.source) || 0) + 1);
      entityDegrees.set(edge.target, (entityDegrees.get(edge.target) || 0) + 1);
    }

    const degrees = Array.from(entityDegrees.values());
    const avgRelationshipsPerEntity = degrees.length > 0
      ? degrees.reduce((a, b) => a + b, 0) / degrees.length
      : 0;

    const maxPossibleEdges = data.nodes.length * (data.nodes.length - 1);
    const densityScore = maxPossibleEdges > 0
      ? data.edges.length / maxPossibleEdges
      : 0;

    return {
      entityTypeDistribution,
      relationshipTypeDistribution,
      avgRelationshipsPerEntity,
      densityScore
    };
  }
}

/**
 * Create an entity relationship generator
 */
export function createEntityRelationshipGenerator(client: OpenRouterClient): EntityRelationshipGenerator {
  return new EntityRelationshipGenerator(client);
}
