"use strict";
/**
 * Custom error types for RuvBot
 */
Object.defineProperty(exports, "__esModule", { value: true });
exports.ErrorCodes = exports.WasmError = exports.AgentError = exports.InitializationError = exports.ConnectionError = exports.RateLimitError = exports.ValidationError = exports.IntegrationError = exports.LLMError = exports.SkillError = exports.MemoryError = exports.SessionError = exports.ConfigurationError = exports.RuvBotError = void 0;
class RuvBotError extends Error {
    constructor(message, code, context) {
        super(message);
        this.name = 'RuvBotError';
        this.code = code;
        this.context = context;
        Error.captureStackTrace(this, this.constructor);
    }
}
exports.RuvBotError = RuvBotError;
class ConfigurationError extends RuvBotError {
    constructor(message, context) {
        super(message, 'CONFIGURATION_ERROR', context);
        this.name = 'ConfigurationError';
    }
}
exports.ConfigurationError = ConfigurationError;
class SessionError extends RuvBotError {
    constructor(message, context) {
        super(message, 'SESSION_ERROR', context);
        this.name = 'SessionError';
    }
}
exports.SessionError = SessionError;
class MemoryError extends RuvBotError {
    constructor(message, context) {
        super(message, 'MEMORY_ERROR', context);
        this.name = 'MemoryError';
    }
}
exports.MemoryError = MemoryError;
class SkillError extends RuvBotError {
    constructor(message, context) {
        super(message, 'SKILL_ERROR', context);
        this.name = 'SkillError';
    }
}
exports.SkillError = SkillError;
class LLMError extends RuvBotError {
    constructor(message, context) {
        super(message, 'LLM_ERROR', context);
        this.name = 'LLMError';
    }
}
exports.LLMError = LLMError;
class IntegrationError extends RuvBotError {
    constructor(message, context) {
        super(message, 'INTEGRATION_ERROR', context);
        this.name = 'IntegrationError';
    }
}
exports.IntegrationError = IntegrationError;
class ValidationError extends RuvBotError {
    constructor(message, errors) {
        super(message, 'VALIDATION_ERROR', { errors });
        this.name = 'ValidationError';
        this.validationErrors = errors;
    }
}
exports.ValidationError = ValidationError;
class RateLimitError extends RuvBotError {
    constructor(message, retryAfter) {
        super(message, 'RATE_LIMIT_ERROR', { retryAfter });
        this.name = 'RateLimitError';
        this.retryAfter = retryAfter;
    }
}
exports.RateLimitError = RateLimitError;
class ConnectionError extends RuvBotError {
    constructor(message, context) {
        super(message, 'CONNECTION_ERROR', context);
        this.name = 'ConnectionError';
    }
}
exports.ConnectionError = ConnectionError;
class InitializationError extends RuvBotError {
    constructor(message, context) {
        super(message, 'INITIALIZATION_ERROR', context);
        this.name = 'InitializationError';
    }
}
exports.InitializationError = InitializationError;
class AgentError extends RuvBotError {
    constructor(message, context) {
        super(message, 'AGENT_ERROR', context);
        this.name = 'AgentError';
    }
}
exports.AgentError = AgentError;
class WasmError extends RuvBotError {
    constructor(message, context) {
        super(message, 'WASM_ERROR', context);
        this.name = 'WasmError';
    }
}
exports.WasmError = WasmError;
/**
 * Error code constants
 */
exports.ErrorCodes = {
    // Configuration
    INVALID_CONFIG: 'INVALID_CONFIG',
    MISSING_API_KEY: 'MISSING_API_KEY',
    INVALID_PROVIDER: 'INVALID_PROVIDER',
    // Session
    SESSION_NOT_FOUND: 'SESSION_NOT_FOUND',
    SESSION_EXPIRED: 'SESSION_EXPIRED',
    SESSION_LIMIT_EXCEEDED: 'SESSION_LIMIT_EXCEEDED',
    // Memory
    MEMORY_FULL: 'MEMORY_FULL',
    EMBEDDING_FAILED: 'EMBEDDING_FAILED',
    INDEX_CORRUPTED: 'INDEX_CORRUPTED',
    // Skills
    SKILL_NOT_FOUND: 'SKILL_NOT_FOUND',
    SKILL_EXECUTION_FAILED: 'SKILL_EXECUTION_FAILED',
    SKILL_TIMEOUT: 'SKILL_TIMEOUT',
    // LLM
    LLM_REQUEST_FAILED: 'LLM_REQUEST_FAILED',
    LLM_TIMEOUT: 'LLM_TIMEOUT',
    CONTEXT_TOO_LONG: 'CONTEXT_TOO_LONG',
    // Integration
    SLACK_CONNECTION_FAILED: 'SLACK_CONNECTION_FAILED',
    DISCORD_CONNECTION_FAILED: 'DISCORD_CONNECTION_FAILED',
    WEBHOOK_DELIVERY_FAILED: 'WEBHOOK_DELIVERY_FAILED',
};
//# sourceMappingURL=errors.js.map