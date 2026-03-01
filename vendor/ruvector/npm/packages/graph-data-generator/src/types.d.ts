/**
 * Core types and interfaces for graph-data-generator
 */
import { z } from 'zod';
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
export interface EntityRelationshipOptions {
    domain: string;
    entityCount: number;
    relationshipDensity: number;
    entitySchema?: Record<string, unknown>;
    relationshipTypes?: string[];
    includeEmbeddings?: boolean;
}
export interface OpenRouterConfig {
    apiKey: string;
    model?: string;
    baseURL?: string;
    timeout?: number;
    maxRetries?: number;
    rateLimit?: {
        requests: number;
        interval: number;
    };
}
export declare const OpenRouterConfigSchema: z.ZodObject<{
    apiKey: z.ZodString;
    model: z.ZodDefault<z.ZodOptional<z.ZodString>>;
    baseURL: z.ZodDefault<z.ZodOptional<z.ZodString>>;
    timeout: z.ZodDefault<z.ZodOptional<z.ZodNumber>>;
    maxRetries: z.ZodDefault<z.ZodOptional<z.ZodNumber>>;
    rateLimit: z.ZodOptional<z.ZodObject<{
        requests: z.ZodNumber;
        interval: z.ZodNumber;
    }, "strip", z.ZodTypeAny, {
        interval: number;
        requests: number;
    }, {
        interval: number;
        requests: number;
    }>>;
}, "strip", z.ZodTypeAny, {
    maxRetries: number;
    apiKey: string;
    model: string;
    timeout: number;
    baseURL: string;
    rateLimit?: {
        interval: number;
        requests: number;
    } | undefined;
}, {
    apiKey: string;
    maxRetries?: number | undefined;
    model?: string | undefined;
    timeout?: number | undefined;
    baseURL?: string | undefined;
    rateLimit?: {
        interval: number;
        requests: number;
    } | undefined;
}>;
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
export declare class GraphGenerationError extends Error {
    code: string;
    details?: unknown | undefined;
    constructor(message: string, code: string, details?: unknown | undefined);
}
export declare class OpenRouterError extends GraphGenerationError {
    constructor(message: string, details?: unknown);
}
export declare class ValidationError extends GraphGenerationError {
    constructor(message: string, details?: unknown);
}
//# sourceMappingURL=types.d.ts.map