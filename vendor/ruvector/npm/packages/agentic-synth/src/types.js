"use strict";
/**
 * Core types and interfaces for agentic-synth
 */
Object.defineProperty(exports, "__esModule", { value: true });
exports.CacheError = exports.APIError = exports.ValidationError = exports.SynthError = exports.EventOptionsSchema = exports.TimeSeriesOptionsSchema = exports.GeneratorOptionsSchema = exports.SynthConfigSchema = exports.DataTypeSchema = exports.CacheStrategySchema = exports.ModelProviderSchema = void 0;
const zod_1 = require("zod");
// Configuration schemas
exports.ModelProviderSchema = zod_1.z.enum(['gemini', 'openrouter']);
exports.CacheStrategySchema = zod_1.z.enum(['none', 'memory', 'disk']);
exports.DataTypeSchema = zod_1.z.enum([
    'timeseries',
    'events',
    'structured',
    'text',
    'json',
    'csv'
]);
exports.SynthConfigSchema = zod_1.z.object({
    provider: exports.ModelProviderSchema,
    apiKey: zod_1.z.string().optional(),
    model: zod_1.z.string().optional(),
    cacheStrategy: exports.CacheStrategySchema.optional().default('memory'),
    cacheTTL: zod_1.z.number().optional().default(3600),
    maxRetries: zod_1.z.number().optional().default(3),
    timeout: zod_1.z.number().optional().default(30000),
    streaming: zod_1.z.boolean().optional().default(false),
    automation: zod_1.z.boolean().optional().default(false),
    vectorDB: zod_1.z.boolean().optional().default(false),
    enableFallback: zod_1.z.boolean().optional().default(true),
    fallbackChain: zod_1.z.array(exports.ModelProviderSchema).optional()
});
exports.GeneratorOptionsSchema = zod_1.z.object({
    count: zod_1.z.number().optional().default(1),
    schema: zod_1.z.record(zod_1.z.string(), zod_1.z.unknown()).optional(),
    format: zod_1.z.enum(['json', 'csv', 'array']).optional().default('json'),
    seed: zod_1.z.union([zod_1.z.string(), zod_1.z.number()]).optional(),
    constraints: zod_1.z.record(zod_1.z.string(), zod_1.z.unknown()).optional()
});
exports.TimeSeriesOptionsSchema = exports.GeneratorOptionsSchema.extend({
    startDate: zod_1.z.union([zod_1.z.date(), zod_1.z.string()]).optional(),
    endDate: zod_1.z.union([zod_1.z.date(), zod_1.z.string()]).optional(),
    interval: zod_1.z.string().optional().default('1h'),
    metrics: zod_1.z.array(zod_1.z.string()).optional(),
    trend: zod_1.z.enum(['up', 'down', 'stable', 'random']).optional().default('stable'),
    seasonality: zod_1.z.boolean().optional().default(false),
    noise: zod_1.z.number().min(0).max(1).optional().default(0.1)
});
exports.EventOptionsSchema = exports.GeneratorOptionsSchema.extend({
    eventTypes: zod_1.z.array(zod_1.z.string()).optional(),
    distribution: zod_1.z.enum(['uniform', 'poisson', 'normal']).optional().default('uniform'),
    timeRange: zod_1.z.object({
        start: zod_1.z.union([zod_1.z.date(), zod_1.z.string()]),
        end: zod_1.z.union([zod_1.z.date(), zod_1.z.string()])
    }).optional(),
    userCount: zod_1.z.number().optional()
});
// Error types
class SynthError extends Error {
    constructor(message, code, details) {
        super(message);
        this.code = code;
        this.details = details;
        this.name = 'SynthError';
    }
}
exports.SynthError = SynthError;
class ValidationError extends SynthError {
    constructor(message, details) {
        super(message, 'VALIDATION_ERROR', details);
        this.name = 'ValidationError';
    }
}
exports.ValidationError = ValidationError;
class APIError extends SynthError {
    constructor(message, details) {
        super(message, 'API_ERROR', details);
        this.name = 'APIError';
    }
}
exports.APIError = APIError;
class CacheError extends SynthError {
    constructor(message, details) {
        super(message, 'CACHE_ERROR', details);
        this.name = 'CacheError';
    }
}
exports.CacheError = CacheError;
//# sourceMappingURL=types.js.map