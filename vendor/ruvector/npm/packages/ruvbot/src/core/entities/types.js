"use strict";
/**
 * Core type definitions for RuvBot entities
 */
Object.defineProperty(exports, "__esModule", { value: true });
exports.TrajectorySchema = exports.TrajectoryStepSchema = exports.TaskSchema = exports.TaskStatusSchema = exports.DomainEventSchema = exports.SkillSchema = exports.SkillParameterSchema = exports.MemoryEntrySchema = exports.MemoryTypeSchema = exports.SessionSchema = exports.MessageSchema = exports.MessageRoleSchema = exports.AgentSchema = exports.AgentConfigSchema = exports.AgentStatusSchema = exports.TenantSchema = void 0;
const zod_1 = require("zod");
// ============================================================================
// Tenant (Multi-tenancy)
// ============================================================================
exports.TenantSchema = zod_1.z.object({
    id: zod_1.z.string().uuid(),
    name: zod_1.z.string().min(1).max(255),
    slug: zod_1.z.string().regex(/^[a-z0-9-]+$/).min(1).max(63),
    settings: zod_1.z.record(zod_1.z.unknown()).default({}),
    createdAt: zod_1.z.date(),
    updatedAt: zod_1.z.date(),
});
// ============================================================================
// Agent
// ============================================================================
exports.AgentStatusSchema = zod_1.z.enum([
    'idle',
    'processing',
    'learning',
    'error',
    'offline',
]);
exports.AgentConfigSchema = zod_1.z.object({
    modelProvider: zod_1.z.enum(['anthropic', 'openai', 'local']).default('anthropic'),
    modelId: zod_1.z.string().default('claude-sonnet-4-20250514'),
    temperature: zod_1.z.number().min(0).max(2).default(0.7),
    maxTokens: zod_1.z.number().min(1).max(200000).default(4096),
    systemPrompt: zod_1.z.string().optional(),
    skills: zod_1.z.array(zod_1.z.string()).default([]),
    learningEnabled: zod_1.z.boolean().default(true),
    memoryEnabled: zod_1.z.boolean().default(true),
});
exports.AgentSchema = zod_1.z.object({
    id: zod_1.z.string().uuid(),
    tenantId: zod_1.z.string().uuid(),
    name: zod_1.z.string().min(1).max(255),
    description: zod_1.z.string().max(1000).optional(),
    status: exports.AgentStatusSchema.default('idle'),
    config: exports.AgentConfigSchema,
    metadata: zod_1.z.record(zod_1.z.unknown()).default({}),
    createdAt: zod_1.z.date(),
    updatedAt: zod_1.z.date(),
});
// ============================================================================
// Session
// ============================================================================
exports.MessageRoleSchema = zod_1.z.enum(['user', 'assistant', 'system', 'tool']);
exports.MessageSchema = zod_1.z.object({
    id: zod_1.z.string().uuid(),
    role: exports.MessageRoleSchema,
    content: zod_1.z.string(),
    toolCalls: zod_1.z.array(zod_1.z.object({
        id: zod_1.z.string(),
        name: zod_1.z.string(),
        arguments: zod_1.z.record(zod_1.z.unknown()),
    })).optional(),
    toolResults: zod_1.z.array(zod_1.z.object({
        toolCallId: zod_1.z.string(),
        result: zod_1.z.unknown(),
        isError: zod_1.z.boolean().default(false),
    })).optional(),
    metadata: zod_1.z.record(zod_1.z.unknown()).default({}),
    createdAt: zod_1.z.date(),
});
exports.SessionSchema = zod_1.z.object({
    id: zod_1.z.string().uuid(),
    tenantId: zod_1.z.string().uuid(),
    agentId: zod_1.z.string().uuid(),
    userId: zod_1.z.string().optional(),
    channelId: zod_1.z.string().optional(),
    channelType: zod_1.z.enum(['slack', 'discord', 'webhook', 'api', 'cli']).optional(),
    messages: zod_1.z.array(exports.MessageSchema).default([]),
    context: zod_1.z.record(zod_1.z.unknown()).default({}),
    isActive: zod_1.z.boolean().default(true),
    expiresAt: zod_1.z.date().optional(),
    createdAt: zod_1.z.date(),
    updatedAt: zod_1.z.date(),
});
// ============================================================================
// Memory
// ============================================================================
exports.MemoryTypeSchema = zod_1.z.enum([
    'episodic', // Specific events/conversations
    'semantic', // General knowledge/facts
    'procedural', // How to do things (skills)
    'working', // Short-term context
]);
exports.MemoryEntrySchema = zod_1.z.object({
    id: zod_1.z.string().uuid(),
    tenantId: zod_1.z.string().uuid(),
    agentId: zod_1.z.string().uuid(),
    type: exports.MemoryTypeSchema,
    content: zod_1.z.string(),
    embedding: zod_1.z.array(zod_1.z.number()).optional(),
    importance: zod_1.z.number().min(0).max(1).default(0.5),
    accessCount: zod_1.z.number().default(0),
    lastAccessedAt: zod_1.z.date().optional(),
    metadata: zod_1.z.record(zod_1.z.unknown()).default({}),
    tags: zod_1.z.array(zod_1.z.string()).default([]),
    expiresAt: zod_1.z.date().optional(),
    createdAt: zod_1.z.date(),
    updatedAt: zod_1.z.date(),
});
// ============================================================================
// Skill
// ============================================================================
exports.SkillParameterSchema = zod_1.z.object({
    name: zod_1.z.string(),
    type: zod_1.z.enum(['string', 'number', 'boolean', 'object', 'array']),
    description: zod_1.z.string(),
    required: zod_1.z.boolean().default(false),
    default: zod_1.z.unknown().optional(),
});
exports.SkillSchema = zod_1.z.object({
    id: zod_1.z.string(),
    name: zod_1.z.string().min(1).max(255),
    description: zod_1.z.string().max(1000),
    version: zod_1.z.string().regex(/^\d+\.\d+\.\d+$/),
    category: zod_1.z.string().default('general'),
    parameters: zod_1.z.array(exports.SkillParameterSchema).default([]),
    examples: zod_1.z.array(zod_1.z.object({
        input: zod_1.z.string(),
        output: zod_1.z.string(),
    })).default([]),
    isBuiltin: zod_1.z.boolean().default(false),
    isEnabled: zod_1.z.boolean().default(true),
    metadata: zod_1.z.record(zod_1.z.unknown()).default({}),
});
// ============================================================================
// Events
// ============================================================================
exports.DomainEventSchema = zod_1.z.object({
    id: zod_1.z.string().uuid(),
    type: zod_1.z.string(),
    tenantId: zod_1.z.string().uuid(),
    aggregateId: zod_1.z.string().uuid(),
    aggregateType: zod_1.z.enum(['agent', 'session', 'memory', 'skill', 'task']),
    payload: zod_1.z.record(zod_1.z.unknown()),
    metadata: zod_1.z.record(zod_1.z.unknown()).default({}),
    timestamp: zod_1.z.date(),
});
// ============================================================================
// Task (Background Workers)
// ============================================================================
exports.TaskStatusSchema = zod_1.z.enum([
    'pending',
    'queued',
    'running',
    'completed',
    'failed',
    'cancelled',
]);
exports.TaskSchema = zod_1.z.object({
    id: zod_1.z.string().uuid(),
    tenantId: zod_1.z.string().uuid(),
    type: zod_1.z.string(),
    priority: zod_1.z.number().min(0).max(100).default(50),
    status: exports.TaskStatusSchema.default('pending'),
    payload: zod_1.z.record(zod_1.z.unknown()),
    result: zod_1.z.unknown().optional(),
    error: zod_1.z.string().optional(),
    attempts: zod_1.z.number().default(0),
    maxAttempts: zod_1.z.number().default(3),
    scheduledAt: zod_1.z.date().optional(),
    startedAt: zod_1.z.date().optional(),
    completedAt: zod_1.z.date().optional(),
    createdAt: zod_1.z.date(),
    updatedAt: zod_1.z.date(),
});
// ============================================================================
// Learning
// ============================================================================
exports.TrajectoryStepSchema = zod_1.z.object({
    action: zod_1.z.string(),
    input: zod_1.z.unknown(),
    output: zod_1.z.unknown(),
    confidence: zod_1.z.number().min(0).max(1),
    duration: zod_1.z.number(),
    timestamp: zod_1.z.date(),
});
exports.TrajectorySchema = zod_1.z.object({
    id: zod_1.z.string().uuid(),
    tenantId: zod_1.z.string().uuid(),
    agentId: zod_1.z.string().uuid(),
    sessionId: zod_1.z.string().uuid(),
    steps: zod_1.z.array(exports.TrajectoryStepSchema),
    outcome: zod_1.z.enum(['success', 'failure', 'partial', 'unknown']),
    reward: zod_1.z.number().min(-1).max(1).default(0),
    metadata: zod_1.z.record(zod_1.z.unknown()).default({}),
    createdAt: zod_1.z.date(),
});
//# sourceMappingURL=types.js.map