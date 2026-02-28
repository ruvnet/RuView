/**
 * Custom error types for RuvBot
 */

export class RuvBotError extends Error {
  public readonly code: string;
  public readonly context?: Record<string, unknown>;

  constructor(message: string, code: string, context?: Record<string, unknown>) {
    super(message);
    this.name = 'RuvBotError';
    this.code = code;
    this.context = context;
    Error.captureStackTrace(this, this.constructor);
  }
}

export class ConfigurationError extends RuvBotError {
  constructor(message: string, context?: Record<string, unknown>) {
    super(message, 'CONFIGURATION_ERROR', context);
    this.name = 'ConfigurationError';
  }
}

export class SessionError extends RuvBotError {
  constructor(message: string, context?: Record<string, unknown>) {
    super(message, 'SESSION_ERROR', context);
    this.name = 'SessionError';
  }
}

export class MemoryError extends RuvBotError {
  constructor(message: string, context?: Record<string, unknown>) {
    super(message, 'MEMORY_ERROR', context);
    this.name = 'MemoryError';
  }
}

export class SkillError extends RuvBotError {
  constructor(message: string, context?: Record<string, unknown>) {
    super(message, 'SKILL_ERROR', context);
    this.name = 'SkillError';
  }
}

export class LLMError extends RuvBotError {
  constructor(message: string, context?: Record<string, unknown>) {
    super(message, 'LLM_ERROR', context);
    this.name = 'LLMError';
  }
}

export class IntegrationError extends RuvBotError {
  constructor(message: string, context?: Record<string, unknown>) {
    super(message, 'INTEGRATION_ERROR', context);
    this.name = 'IntegrationError';
  }
}

export class ValidationError extends RuvBotError {
  public readonly validationErrors: ValidationErrorDetail[];

  constructor(message: string, errors: ValidationErrorDetail[]) {
    super(message, 'VALIDATION_ERROR', { errors });
    this.name = 'ValidationError';
    this.validationErrors = errors;
  }
}

export interface ValidationErrorDetail {
  field: string;
  message: string;
  value?: unknown;
}

export class RateLimitError extends RuvBotError {
  public readonly retryAfter?: number;

  constructor(message: string, retryAfter?: number) {
    super(message, 'RATE_LIMIT_ERROR', { retryAfter });
    this.name = 'RateLimitError';
    this.retryAfter = retryAfter;
  }
}

export class ConnectionError extends RuvBotError {
  constructor(message: string, context?: Record<string, unknown>) {
    super(message, 'CONNECTION_ERROR', context);
    this.name = 'ConnectionError';
  }
}

export class InitializationError extends RuvBotError {
  constructor(message: string, context?: Record<string, unknown>) {
    super(message, 'INITIALIZATION_ERROR', context);
    this.name = 'InitializationError';
  }
}

export class AgentError extends RuvBotError {
  constructor(message: string, context?: Record<string, unknown>) {
    super(message, 'AGENT_ERROR', context);
    this.name = 'AgentError';
  }
}

export class WasmError extends RuvBotError {
  constructor(message: string, context?: Record<string, unknown>) {
    super(message, 'WASM_ERROR', context);
    this.name = 'WasmError';
  }
}

/**
 * Error code constants
 */
export const ErrorCodes = {
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
} as const;

export type ErrorCode = (typeof ErrorCodes)[keyof typeof ErrorCodes];
