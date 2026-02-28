"use strict";
/**
 * Core types and interfaces for graph-data-generator
 */
Object.defineProperty(exports, "__esModule", { value: true });
exports.ValidationError = exports.OpenRouterError = exports.GraphGenerationError = exports.OpenRouterConfigSchema = void 0;
const zod_1 = require("zod");
exports.OpenRouterConfigSchema = zod_1.z.object({
    apiKey: zod_1.z.string(),
    model: zod_1.z.string().optional().default('moonshot/kimi-k2-instruct'),
    baseURL: zod_1.z.string().optional().default('https://openrouter.ai/api/v1'),
    timeout: zod_1.z.number().optional().default(60000),
    maxRetries: zod_1.z.number().optional().default(3),
    rateLimit: zod_1.z.object({
        requests: zod_1.z.number(),
        interval: zod_1.z.number()
    }).optional()
});
// Error types
class GraphGenerationError extends Error {
    constructor(message, code, details) {
        super(message);
        this.code = code;
        this.details = details;
        this.name = 'GraphGenerationError';
    }
}
exports.GraphGenerationError = GraphGenerationError;
class OpenRouterError extends GraphGenerationError {
    constructor(message, details) {
        super(message, 'OPENROUTER_ERROR', details);
        this.name = 'OpenRouterError';
    }
}
exports.OpenRouterError = OpenRouterError;
class ValidationError extends GraphGenerationError {
    constructor(message, details) {
        super(message, 'VALIDATION_ERROR', details);
        this.name = 'ValidationError';
    }
}
exports.ValidationError = ValidationError;
//# sourceMappingURL=types.js.map