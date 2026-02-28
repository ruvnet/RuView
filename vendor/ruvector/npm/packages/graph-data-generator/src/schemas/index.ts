/**
 * Zod schemas for graph data validation
 */

import { z } from 'zod';

// Graph node schema
export const GraphNodeSchema = z.object({
  id: z.string(),
  labels: z.array(z.string()),
  properties: z.record(z.string(), z.unknown()),
  embedding: z.array(z.number()).optional()
});

// Graph edge schema
export const GraphEdgeSchema = z.object({
  id: z.string(),
  type: z.string(),
  source: z.string(),
  target: z.string(),
  properties: z.record(z.string(), z.unknown()),
  embedding: z.array(z.number()).optional()
});

// Graph data schema
export const GraphDataSchema = z.object({
  nodes: z.array(GraphNodeSchema),
  edges: z.array(GraphEdgeSchema),
  metadata: z.object({
    domain: z.string().optional(),
    generated_at: z.date().optional(),
    model: z.string().optional(),
    total_nodes: z.number().optional(),
    total_edges: z.number().optional()
  }).optional()
});

// Knowledge graph options schema
export const KnowledgeGraphOptionsSchema = z.object({
  domain: z.string(),
  entities: z.number().positive(),
  relationships: z.number().positive(),
  entityTypes: z.array(z.string()).optional(),
  relationshipTypes: z.array(z.string()).optional(),
  includeEmbeddings: z.boolean().optional(),
  embeddingDimension: z.number().positive().optional()
});

// Social network options schema
export const SocialNetworkOptionsSchema = z.object({
  users: z.number().positive(),
  avgConnections: z.number().positive(),
  networkType: z.enum(['random', 'small-world', 'scale-free', 'clustered']).optional(),
  communities: z.number().positive().optional(),
  includeMetadata: z.boolean().optional(),
  includeEmbeddings: z.boolean().optional()
});

// Temporal event options schema
export const TemporalEventOptionsSchema = z.object({
  startDate: z.union([z.date(), z.string()]),
  endDate: z.union([z.date(), z.string()]),
  eventTypes: z.array(z.string()),
  eventsPerDay: z.number().positive().optional(),
  entities: z.number().positive().optional(),
  includeEmbeddings: z.boolean().optional()
});

// Entity relationship options schema
export const EntityRelationshipOptionsSchema = z.object({
  domain: z.string(),
  entityCount: z.number().positive(),
  relationshipDensity: z.number().min(0).max(1),
  entitySchema: z.record(z.string(), z.unknown()).optional(),
  relationshipTypes: z.array(z.string()).optional(),
  includeEmbeddings: z.boolean().optional()
});

// Cypher statement schema
export const CypherStatementSchema = z.object({
  query: z.string(),
  parameters: z.record(z.string(), z.unknown()).optional()
});

// Cypher batch schema
export const CypherBatchSchema = z.object({
  statements: z.array(CypherStatementSchema),
  metadata: z.object({
    total_nodes: z.number().optional(),
    total_relationships: z.number().optional(),
    labels: z.array(z.string()).optional(),
    relationship_types: z.array(z.string()).optional()
  }).optional()
});

// Graph generation result schema
export const GraphGenerationResultSchema = z.object({
  data: GraphDataSchema,
  metadata: z.object({
    generated_at: z.date(),
    model: z.string(),
    duration: z.number(),
    token_usage: z.object({
      prompt_tokens: z.number(),
      completion_tokens: z.number(),
      total_tokens: z.number()
    }).optional()
  }),
  cypher: CypherBatchSchema.optional()
});

// Validation helpers
export function validateGraphData(data: unknown) {
  return GraphDataSchema.parse(data);
}

export function validateKnowledgeGraphOptions(options: unknown) {
  return KnowledgeGraphOptionsSchema.parse(options);
}

export function validateSocialNetworkOptions(options: unknown) {
  return SocialNetworkOptionsSchema.parse(options);
}

export function validateTemporalEventOptions(options: unknown) {
  return TemporalEventOptionsSchema.parse(options);
}

export function validateEntityRelationshipOptions(options: unknown) {
  return EntityRelationshipOptionsSchema.parse(options);
}

export function validateCypherBatch(batch: unknown) {
  return CypherBatchSchema.parse(batch);
}

export function validateGraphGenerationResult(result: unknown) {
  return GraphGenerationResultSchema.parse(result);
}
