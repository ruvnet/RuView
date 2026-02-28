/**
 * Core types and interfaces for graph-data-generator
 */

import { z } from 'zod';

// Graph data types
export interface GraphNode {
  id: string;
  labels: string[];
  properties: Record<string, unknown>;
  embedding?: number[];
}

export interface GraphEdge {
  id: string;
  type: string;
  source: string;
  target: string;
  properties: Record<string, unknown>;
  embedding?: number[];
}

export interface GraphData {
  nodes: GraphNode[];
  edges: GraphEdge[];
  metadata?: {
    domain?: string;
    generated_at?: Date;
    model?: string;
    total_nodes?: number;
    total_edges?: number;
  };
}

// Knowledge graph types
export interface KnowledgeTriple {
  subject: string;
  predicate: string;
  object: string;
  confidence?: number;
  source?: string;
}

export interface KnowledgeGraphOptions {
  domain: string;
  entities: number;
  relationships: number;
  entityTypes?: string[];
  relationshipTypes?: string[];
  includeEmbeddings?: boolean;
  embeddingDimension?: number;
}

// Social network types
export interface SocialNetworkOptions {
  users: number;
  avgConnections: number;
  networkType?: 'random' | 'small-world' | 'scale-free' | 'clustered';
  communities?: number;
  includeMetadata?: boolean;
  includeEmbeddings?: boolean;
}

export interface SocialNode {
  id: string;
  username: string;
  profile: {
    name?: string;
    bio?: string;
    joined?: Date;
    followers?: number;
    following?: number;
  };
  metadata?: Record<string, unknown>;
}

// Temporal event types
export interface TemporalEventOptions {
  startDate: Date | string;
  endDate: Date | string;
  eventTypes: string[];
  eventsPerDay?: number;
  entities?: number;
  includeEmbeddings?: boolean;
}

export interface TemporalEvent {
  id: string;
  type: string;
  timestamp: Date;
  entities: string[];
  properties: Record<string, unknown>;
  relationships?: Array<{
    type: string;
    target: string;
  }>;
}

// Entity relationship types
export interface EntityRelationshipOptions {
  domain: string;
  entityCount: number;
  relationshipDensity: number; // 0-1
  entitySchema?: Record<string, unknown>;
  relationshipTypes?: string[];
  includeEmbeddings?: boolean;
}

// OpenRouter client types
export interface OpenRouterConfig {
  apiKey: string;
  model?: string;
  baseURL?: string;
  timeout?: number;
  maxRetries?: number;
  rateLimit?: {
    requests: number;
    interval: number; // milliseconds
  };
}

export const OpenRouterConfigSchema = z.object({
  apiKey: z.string(),
  model: z.string().optional().default('moonshot/kimi-k2-instruct'),
  baseURL: z.string().optional().default('https://openrouter.ai/api/v1'),
  timeout: z.number().optional().default(60000),
  maxRetries: z.number().optional().default(3),
  rateLimit: z.object({
    requests: z.number(),
    interval: z.number()
  }).optional()
});

export interface OpenRouterMessage {
  role: 'system' | 'user' | 'assistant';
  content: string;
}

export interface OpenRouterRequest {
  model: string;
  messages: OpenRouterMessage[];
  temperature?: number;
  max_tokens?: number;
  top_p?: number;
  stream?: boolean;
}

export interface OpenRouterResponse {
  id: string;
  model: string;
  choices: Array<{
    message: {
      role: string;
      content: string;
    };
    finish_reason: string;
  }>;
  usage: {
    prompt_tokens: number;
    completion_tokens: number;
    total_tokens: number;
  };
}

// Cypher types
export interface CypherStatement {
  query: string;
  parameters?: Record<string, unknown>;
}

export interface CypherBatch {
  statements: CypherStatement[];
  metadata?: {
    total_nodes?: number;
    total_relationships?: number;
    labels?: string[];
    relationship_types?: string[];
  };
}

// Embedding types
export interface EmbeddingConfig {
  provider: 'openrouter' | 'local';
  model?: string;
  dimensions?: number;
  batchSize?: number;
}

export interface EmbeddingResult {
  embedding: number[];
  model: string;
  dimensions: number;
}

// Generation result types
export interface GraphGenerationResult<T = GraphData> {
  data: T;
  metadata: {
    generated_at: Date;
    model: string;
    duration: number;
    token_usage?: {
      prompt_tokens: number;
      completion_tokens: number;
      total_tokens: number;
    };
  };
  cypher?: CypherBatch;
}

// Error types
export class GraphGenerationError extends Error {
  constructor(
    message: string,
    public code: string,
    public details?: unknown
  ) {
    super(message);
    this.name = 'GraphGenerationError';
  }
}

export class OpenRouterError extends GraphGenerationError {
  constructor(message: string, details?: unknown) {
    super(message, 'OPENROUTER_ERROR', details);
    this.name = 'OpenRouterError';
  }
}

export class ValidationError extends GraphGenerationError {
  constructor(message: string, details?: unknown) {
    super(message, 'VALIDATION_ERROR', details);
    this.name = 'ValidationError';
  }
}
