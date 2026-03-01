/**
 * Core types and interfaces for agentic-synth
 */

import { z } from 'zod';

// JSON types
export type JsonPrimitive = string | number | boolean | null;
export type JsonArray = JsonValue[];
export type JsonObject = { [key: string]: JsonValue };
export type JsonValue = JsonPrimitive | JsonArray | JsonObject;

// Schema types
export interface SchemaField {
  type: string;
  required?: boolean;
  properties?: Record<string, SchemaField>;
  items?: SchemaField;
  enum?: unknown[];
  minimum?: number;
  maximum?: number;
  pattern?: string;
}

export type DataSchema = Record<string, SchemaField>;
export type DataConstraints = Record<string, unknown>;

// Configuration schemas
export const ModelProviderSchema = z.enum(['gemini', 'openrouter']);
export type ModelProvider = z.infer<typeof ModelProviderSchema>;

export const CacheStrategySchema = z.enum(['none', 'memory', 'disk']);
export type CacheStrategy = z.infer<typeof CacheStrategySchema>;

export const DataTypeSchema = z.enum([
  'timeseries',
  'events',
  'structured',
  'text',
  'json',
  'csv'
]);
export type DataType = z.infer<typeof DataTypeSchema>;

// Configuration interface
export interface SynthConfig {
  provider: ModelProvider;
  apiKey?: string;
  model?: string;
  cacheStrategy?: CacheStrategy;
  cacheTTL?: number;
  maxRetries?: number;
  timeout?: number;
  streaming?: boolean;
  automation?: boolean;
  vectorDB?: boolean;
  enableFallback?: boolean;
  fallbackChain?: ModelProvider[];
}

export const SynthConfigSchema = z.object({
  provider: ModelProviderSchema,
  apiKey: z.string().optional(),
  model: z.string().optional(),
  cacheStrategy: CacheStrategySchema.optional().default('memory'),
  cacheTTL: z.number().optional().default(3600),
  maxRetries: z.number().optional().default(3),
  timeout: z.number().optional().default(30000),
  streaming: z.boolean().optional().default(false),
  automation: z.boolean().optional().default(false),
  vectorDB: z.boolean().optional().default(false),
  enableFallback: z.boolean().optional().default(true),
  fallbackChain: z.array(ModelProviderSchema).optional()
});

// Generator options
export interface GeneratorOptions {
  count?: number;
  schema?: DataSchema;
  format?: 'json' | 'csv' | 'array';
  seed?: string | number;
  constraints?: DataConstraints;
}

export const GeneratorOptionsSchema = z.object({
  count: z.number().optional().default(1),
  schema: z.record(z.string(), z.unknown()).optional(),
  format: z.enum(['json', 'csv', 'array']).optional().default('json'),
  seed: z.union([z.string(), z.number()]).optional(),
  constraints: z.record(z.string(), z.unknown()).optional()
});

// Time series specific options
export interface TimeSeriesOptions extends GeneratorOptions {
  startDate?: Date | string;
  endDate?: Date | string;
  interval?: string; // e.g., '1h', '1d', '5m'
  metrics?: string[];
  trend?: 'up' | 'down' | 'stable' | 'random';
  seasonality?: boolean;
  noise?: number; // 0-1
}

export const TimeSeriesOptionsSchema = GeneratorOptionsSchema.extend({
  startDate: z.union([z.date(), z.string()]).optional(),
  endDate: z.union([z.date(), z.string()]).optional(),
  interval: z.string().optional().default('1h'),
  metrics: z.array(z.string()).optional(),
  trend: z.enum(['up', 'down', 'stable', 'random']).optional().default('stable'),
  seasonality: z.boolean().optional().default(false),
  noise: z.number().min(0).max(1).optional().default(0.1)
});

// Event specific options
export interface EventOptions extends GeneratorOptions {
  eventTypes?: string[];
  distribution?: 'uniform' | 'poisson' | 'normal';
  timeRange?: {
    start: Date | string;
    end: Date | string;
  };
  userCount?: number;
}

export const EventOptionsSchema = GeneratorOptionsSchema.extend({
  eventTypes: z.array(z.string()).optional(),
  distribution: z.enum(['uniform', 'poisson', 'normal']).optional().default('uniform'),
  timeRange: z.object({
    start: z.union([z.date(), z.string()]),
    end: z.union([z.date(), z.string()])
  }).optional(),
  userCount: z.number().optional()
});

// Generation result
export interface GenerationResult<T = JsonValue> {
  data: T[];
  metadata: {
    count: number;
    generatedAt: Date;
    provider: ModelProvider;
    model: string;
    cached: boolean;
    duration: number;
  };
}

// Error types
export class SynthError extends Error {
  constructor(
    message: string,
    public code: string,
    public details?: unknown
  ) {
    super(message);
    this.name = 'SynthError';
  }
}

export class ValidationError extends SynthError {
  constructor(message: string, details?: unknown) {
    super(message, 'VALIDATION_ERROR', details);
    this.name = 'ValidationError';
  }
}

export class APIError extends SynthError {
  constructor(message: string, details?: unknown) {
    super(message, 'API_ERROR', details);
    this.name = 'APIError';
  }
}

export class CacheError extends SynthError {
  constructor(message: string, details?: unknown) {
    super(message, 'CACHE_ERROR', details);
    this.name = 'CacheError';
  }
}

// Model routing
export interface ModelRoute {
  provider: ModelProvider;
  model: string;
  priority: number;
  capabilities: string[];
}

// Streaming types
export interface StreamChunk<T = JsonValue> {
  type: 'data' | 'metadata' | 'error' | 'complete';
  data?: T;
  metadata?: Record<string, unknown>;
  error?: Error;
}

export type StreamCallback<T = JsonValue> = (chunk: StreamChunk<T>) => void | Promise<void>;
