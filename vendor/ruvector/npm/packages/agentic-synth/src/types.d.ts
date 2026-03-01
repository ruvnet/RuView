/**
 * Core types and interfaces for agentic-synth
 */
import { z } from 'zod';
export type JsonPrimitive = string | number | boolean | null;
export type JsonArray = JsonValue[];
export type JsonObject = {
    [key: string]: JsonValue;
};
export type JsonValue = JsonPrimitive | JsonArray | JsonObject;
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
export declare const ModelProviderSchema: z.ZodEnum<["gemini", "openrouter"]>;
export type ModelProvider = z.infer<typeof ModelProviderSchema>;
export declare const CacheStrategySchema: z.ZodEnum<["none", "memory", "disk"]>;
export type CacheStrategy = z.infer<typeof CacheStrategySchema>;
export declare const DataTypeSchema: z.ZodEnum<["timeseries", "events", "structured", "text", "json", "csv"]>;
export type DataType = z.infer<typeof DataTypeSchema>;
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
export declare const SynthConfigSchema: z.ZodObject<{
    provider: z.ZodEnum<["gemini", "openrouter"]>;
    apiKey: z.ZodOptional<z.ZodString>;
    model: z.ZodOptional<z.ZodString>;
    cacheStrategy: z.ZodDefault<z.ZodOptional<z.ZodEnum<["none", "memory", "disk"]>>>;
    cacheTTL: z.ZodDefault<z.ZodOptional<z.ZodNumber>>;
    maxRetries: z.ZodDefault<z.ZodOptional<z.ZodNumber>>;
    timeout: z.ZodDefault<z.ZodOptional<z.ZodNumber>>;
    streaming: z.ZodDefault<z.ZodOptional<z.ZodBoolean>>;
    automation: z.ZodDefault<z.ZodOptional<z.ZodBoolean>>;
    vectorDB: z.ZodDefault<z.ZodOptional<z.ZodBoolean>>;
    enableFallback: z.ZodDefault<z.ZodOptional<z.ZodBoolean>>;
    fallbackChain: z.ZodOptional<z.ZodArray<z.ZodEnum<["gemini", "openrouter"]>, "many">>;
}, "strip", z.ZodTypeAny, {
    maxRetries: number;
    provider: "gemini" | "openrouter";
    cacheStrategy: "none" | "memory" | "disk";
    cacheTTL: number;
    timeout: number;
    streaming: boolean;
    automation: boolean;
    vectorDB: boolean;
    enableFallback: boolean;
    apiKey?: string | undefined;
    model?: string | undefined;
    fallbackChain?: ("gemini" | "openrouter")[] | undefined;
}, {
    provider: "gemini" | "openrouter";
    maxRetries?: number | undefined;
    apiKey?: string | undefined;
    model?: string | undefined;
    cacheStrategy?: "none" | "memory" | "disk" | undefined;
    cacheTTL?: number | undefined;
    timeout?: number | undefined;
    streaming?: boolean | undefined;
    automation?: boolean | undefined;
    vectorDB?: boolean | undefined;
    enableFallback?: boolean | undefined;
    fallbackChain?: ("gemini" | "openrouter")[] | undefined;
}>;
export interface GeneratorOptions {
    count?: number;
    schema?: DataSchema;
    format?: 'json' | 'csv' | 'array';
    seed?: string | number;
    constraints?: DataConstraints;
}
export declare const GeneratorOptionsSchema: z.ZodObject<{
    count: z.ZodDefault<z.ZodOptional<z.ZodNumber>>;
    schema: z.ZodOptional<z.ZodRecord<z.ZodString, z.ZodUnknown>>;
    format: z.ZodDefault<z.ZodOptional<z.ZodEnum<["json", "csv", "array"]>>>;
    seed: z.ZodOptional<z.ZodUnion<[z.ZodString, z.ZodNumber]>>;
    constraints: z.ZodOptional<z.ZodRecord<z.ZodString, z.ZodUnknown>>;
}, "strip", z.ZodTypeAny, {
    count: number;
    format: "json" | "csv" | "array";
    schema?: Record<string, unknown> | undefined;
    seed?: string | number | undefined;
    constraints?: Record<string, unknown> | undefined;
}, {
    count?: number | undefined;
    schema?: Record<string, unknown> | undefined;
    format?: "json" | "csv" | "array" | undefined;
    seed?: string | number | undefined;
    constraints?: Record<string, unknown> | undefined;
}>;
export interface TimeSeriesOptions extends GeneratorOptions {
    startDate?: Date | string;
    endDate?: Date | string;
    interval?: string;
    metrics?: string[];
    trend?: 'up' | 'down' | 'stable' | 'random';
    seasonality?: boolean;
    noise?: number;
}
export declare const TimeSeriesOptionsSchema: z.ZodObject<{
    count: z.ZodDefault<z.ZodOptional<z.ZodNumber>>;
    schema: z.ZodOptional<z.ZodRecord<z.ZodString, z.ZodUnknown>>;
    format: z.ZodDefault<z.ZodOptional<z.ZodEnum<["json", "csv", "array"]>>>;
    seed: z.ZodOptional<z.ZodUnion<[z.ZodString, z.ZodNumber]>>;
    constraints: z.ZodOptional<z.ZodRecord<z.ZodString, z.ZodUnknown>>;
} & {
    startDate: z.ZodOptional<z.ZodUnion<[z.ZodDate, z.ZodString]>>;
    endDate: z.ZodOptional<z.ZodUnion<[z.ZodDate, z.ZodString]>>;
    interval: z.ZodDefault<z.ZodOptional<z.ZodString>>;
    metrics: z.ZodOptional<z.ZodArray<z.ZodString, "many">>;
    trend: z.ZodDefault<z.ZodOptional<z.ZodEnum<["up", "down", "stable", "random"]>>>;
    seasonality: z.ZodDefault<z.ZodOptional<z.ZodBoolean>>;
    noise: z.ZodDefault<z.ZodOptional<z.ZodNumber>>;
}, "strip", z.ZodTypeAny, {
    count: number;
    format: "json" | "csv" | "array";
    interval: string;
    trend: "up" | "down" | "stable" | "random";
    seasonality: boolean;
    noise: number;
    metrics?: string[] | undefined;
    schema?: Record<string, unknown> | undefined;
    seed?: string | number | undefined;
    constraints?: Record<string, unknown> | undefined;
    startDate?: string | Date | undefined;
    endDate?: string | Date | undefined;
}, {
    metrics?: string[] | undefined;
    count?: number | undefined;
    schema?: Record<string, unknown> | undefined;
    format?: "json" | "csv" | "array" | undefined;
    seed?: string | number | undefined;
    constraints?: Record<string, unknown> | undefined;
    startDate?: string | Date | undefined;
    endDate?: string | Date | undefined;
    interval?: string | undefined;
    trend?: "up" | "down" | "stable" | "random" | undefined;
    seasonality?: boolean | undefined;
    noise?: number | undefined;
}>;
export interface EventOptions extends GeneratorOptions {
    eventTypes?: string[];
    distribution?: 'uniform' | 'poisson' | 'normal';
    timeRange?: {
        start: Date | string;
        end: Date | string;
    };
    userCount?: number;
}
export declare const EventOptionsSchema: z.ZodObject<{
    count: z.ZodDefault<z.ZodOptional<z.ZodNumber>>;
    schema: z.ZodOptional<z.ZodRecord<z.ZodString, z.ZodUnknown>>;
    format: z.ZodDefault<z.ZodOptional<z.ZodEnum<["json", "csv", "array"]>>>;
    seed: z.ZodOptional<z.ZodUnion<[z.ZodString, z.ZodNumber]>>;
    constraints: z.ZodOptional<z.ZodRecord<z.ZodString, z.ZodUnknown>>;
} & {
    eventTypes: z.ZodOptional<z.ZodArray<z.ZodString, "many">>;
    distribution: z.ZodDefault<z.ZodOptional<z.ZodEnum<["uniform", "poisson", "normal"]>>>;
    timeRange: z.ZodOptional<z.ZodObject<{
        start: z.ZodUnion<[z.ZodDate, z.ZodString]>;
        end: z.ZodUnion<[z.ZodDate, z.ZodString]>;
    }, "strip", z.ZodTypeAny, {
        start: string | Date;
        end: string | Date;
    }, {
        start: string | Date;
        end: string | Date;
    }>>;
    userCount: z.ZodOptional<z.ZodNumber>;
}, "strip", z.ZodTypeAny, {
    count: number;
    format: "json" | "csv" | "array";
    distribution: "uniform" | "poisson" | "normal";
    schema?: Record<string, unknown> | undefined;
    seed?: string | number | undefined;
    constraints?: Record<string, unknown> | undefined;
    eventTypes?: string[] | undefined;
    timeRange?: {
        start: string | Date;
        end: string | Date;
    } | undefined;
    userCount?: number | undefined;
}, {
    count?: number | undefined;
    schema?: Record<string, unknown> | undefined;
    format?: "json" | "csv" | "array" | undefined;
    seed?: string | number | undefined;
    constraints?: Record<string, unknown> | undefined;
    eventTypes?: string[] | undefined;
    distribution?: "uniform" | "poisson" | "normal" | undefined;
    timeRange?: {
        start: string | Date;
        end: string | Date;
    } | undefined;
    userCount?: number | undefined;
}>;
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
export declare class SynthError extends Error {
    code: string;
    details?: unknown | undefined;
    constructor(message: string, code: string, details?: unknown | undefined);
}
export declare class ValidationError extends SynthError {
    constructor(message: string, details?: unknown);
}
export declare class APIError extends SynthError {
    constructor(message: string, details?: unknown);
}
export declare class CacheError extends SynthError {
    constructor(message: string, details?: unknown);
}
export interface ModelRoute {
    provider: ModelProvider;
    model: string;
    priority: number;
    capabilities: string[];
}
export interface StreamChunk<T = JsonValue> {
    type: 'data' | 'metadata' | 'error' | 'complete';
    data?: T;
    metadata?: Record<string, unknown>;
    error?: Error;
}
export type StreamCallback<T = JsonValue> = (chunk: StreamChunk<T>) => void | Promise<void>;
//# sourceMappingURL=types.d.ts.map