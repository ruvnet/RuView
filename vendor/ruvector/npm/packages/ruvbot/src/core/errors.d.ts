/**
 * Custom error types for RuvBot
 */
export declare class RuvBotError extends Error {
    readonly code: string;
    readonly context?: Record<string, unknown>;
    constructor(message: string, code: string, context?: Record<string, unknown>);
}
export declare class ConfigurationError extends RuvBotError {
    constructor(message: string, context?: Record<string, unknown>);
}
export declare class SessionError extends RuvBotError {
    constructor(message: string, context?: Record<string, unknown>);
}
export declare class MemoryError extends RuvBotError {
    constructor(message: string, context?: Record<string, unknown>);
}
export declare class SkillError extends RuvBotError {
    constructor(message: string, context?: Record<string, unknown>);
}
export declare class LLMError extends RuvBotError {
    constructor(message: string, context?: Record<string, unknown>);
}
export declare class IntegrationError extends RuvBotError {
    constructor(message: string, context?: Record<string, unknown>);
}
export declare class ValidationError extends RuvBotError {
    readonly validationErrors: ValidationErrorDetail[];
    constructor(message: string, errors: ValidationErrorDetail[]);
}
export interface ValidationErrorDetail {
    field: string;
    message: string;
    value?: unknown;
}
export declare class RateLimitError extends RuvBotError {
    readonly retryAfter?: number;
    constructor(message: string, retryAfter?: number);
}
export declare class ConnectionError extends RuvBotError {
    constructor(message: string, context?: Record<string, unknown>);
}
export declare class InitializationError extends RuvBotError {
    constructor(message: string, context?: Record<string, unknown>);
}
export declare class AgentError extends RuvBotError {
    constructor(message: string, context?: Record<string, unknown>);
}
export declare class WasmError extends RuvBotError {
    constructor(message: string, context?: Record<string, unknown>);
}
/**
 * Error code constants
 */
export declare const ErrorCodes: {
    readonly INVALID_CONFIG: "INVALID_CONFIG";
    readonly MISSING_API_KEY: "MISSING_API_KEY";
    readonly INVALID_PROVIDER: "INVALID_PROVIDER";
    readonly SESSION_NOT_FOUND: "SESSION_NOT_FOUND";
    readonly SESSION_EXPIRED: "SESSION_EXPIRED";
    readonly SESSION_LIMIT_EXCEEDED: "SESSION_LIMIT_EXCEEDED";
    readonly MEMORY_FULL: "MEMORY_FULL";
    readonly EMBEDDING_FAILED: "EMBEDDING_FAILED";
    readonly INDEX_CORRUPTED: "INDEX_CORRUPTED";
    readonly SKILL_NOT_FOUND: "SKILL_NOT_FOUND";
    readonly SKILL_EXECUTION_FAILED: "SKILL_EXECUTION_FAILED";
    readonly SKILL_TIMEOUT: "SKILL_TIMEOUT";
    readonly LLM_REQUEST_FAILED: "LLM_REQUEST_FAILED";
    readonly LLM_TIMEOUT: "LLM_TIMEOUT";
    readonly CONTEXT_TOO_LONG: "CONTEXT_TOO_LONG";
    readonly SLACK_CONNECTION_FAILED: "SLACK_CONNECTION_FAILED";
    readonly DISCORD_CONNECTION_FAILED: "DISCORD_CONNECTION_FAILED";
    readonly WEBHOOK_DELIVERY_FAILED: "WEBHOOK_DELIVERY_FAILED";
};
export type ErrorCode = (typeof ErrorCodes)[keyof typeof ErrorCodes];
//# sourceMappingURL=errors.d.ts.map