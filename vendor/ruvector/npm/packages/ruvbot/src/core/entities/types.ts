/**
 * Core type definitions for RuvBot entities
 */

import { z } from 'zod';

// ============================================================================
// Tenant (Multi-tenancy)
// ============================================================================

export const TenantSchema = z.object({
  id: z.string().uuid(),
  name: z.string().min(1).max(255),
  slug: z.string().regex(/^[a-z0-9-]+$/).min(1).max(63),
  settings: z.record(z.unknown()).default({}),
  createdAt: z.date(),
  updatedAt: z.date(),
});

export type Tenant = z.infer<typeof TenantSchema>;

// ============================================================================
// Agent
// ============================================================================

export const AgentStatusSchema = z.enum([
  'idle',
  'processing',
  'learning',
  'error',
  'offline',
]);

export type AgentStatus = z.infer<typeof AgentStatusSchema>;

export const AgentConfigSchema = z.object({
  modelProvider: z.enum(['anthropic', 'openai', 'local']).default('anthropic'),
  modelId: z.string().default('claude-sonnet-4-20250514'),
  temperature: z.number().min(0).max(2).default(0.7),
  maxTokens: z.number().min(1).max(200000).default(4096),
  systemPrompt: z.string().optional(),
  skills: z.array(z.string()).default([]),
  learningEnabled: z.boolean().default(true),
  memoryEnabled: z.boolean().default(true),
});

export type AgentConfig = z.infer<typeof AgentConfigSchema>;

export const AgentSchema = z.object({
  id: z.string().uuid(),
  tenantId: z.string().uuid(),
  name: z.string().min(1).max(255),
  description: z.string().max(1000).optional(),
  status: AgentStatusSchema.default('idle'),
  config: AgentConfigSchema,
  metadata: z.record(z.unknown()).default({}),
  createdAt: z.date(),
  updatedAt: z.date(),
});

export type Agent = z.infer<typeof AgentSchema>;

// ============================================================================
// Session
// ============================================================================

export const MessageRoleSchema = z.enum(['user', 'assistant', 'system', 'tool']);
export type MessageRole = z.infer<typeof MessageRoleSchema>;

export const MessageSchema = z.object({
  id: z.string().uuid(),
  role: MessageRoleSchema,
  content: z.string(),
  toolCalls: z.array(z.object({
    id: z.string(),
    name: z.string(),
    arguments: z.record(z.unknown()),
  })).optional(),
  toolResults: z.array(z.object({
    toolCallId: z.string(),
    result: z.unknown(),
    isError: z.boolean().default(false),
  })).optional(),
  metadata: z.record(z.unknown()).default({}),
  createdAt: z.date(),
});

export type Message = z.infer<typeof MessageSchema>;

export const SessionSchema = z.object({
  id: z.string().uuid(),
  tenantId: z.string().uuid(),
  agentId: z.string().uuid(),
  userId: z.string().optional(),
  channelId: z.string().optional(),
  channelType: z.enum(['slack', 'discord', 'webhook', 'api', 'cli']).optional(),
  messages: z.array(MessageSchema).default([]),
  context: z.record(z.unknown()).default({}),
  isActive: z.boolean().default(true),
  expiresAt: z.date().optional(),
  createdAt: z.date(),
  updatedAt: z.date(),
});

export type Session = z.infer<typeof SessionSchema>;

// ============================================================================
// Memory
// ============================================================================

export const MemoryTypeSchema = z.enum([
  'episodic',    // Specific events/conversations
  'semantic',    // General knowledge/facts
  'procedural',  // How to do things (skills)
  'working',     // Short-term context
]);

export type MemoryType = z.infer<typeof MemoryTypeSchema>;

export const MemoryEntrySchema = z.object({
  id: z.string().uuid(),
  tenantId: z.string().uuid(),
  agentId: z.string().uuid(),
  type: MemoryTypeSchema,
  content: z.string(),
  embedding: z.array(z.number()).optional(),
  importance: z.number().min(0).max(1).default(0.5),
  accessCount: z.number().default(0),
  lastAccessedAt: z.date().optional(),
  metadata: z.record(z.unknown()).default({}),
  tags: z.array(z.string()).default([]),
  expiresAt: z.date().optional(),
  createdAt: z.date(),
  updatedAt: z.date(),
});

export type MemoryEntry = z.infer<typeof MemoryEntrySchema>;

// ============================================================================
// Skill
// ============================================================================

export const SkillParameterSchema = z.object({
  name: z.string(),
  type: z.enum(['string', 'number', 'boolean', 'object', 'array']),
  description: z.string(),
  required: z.boolean().default(false),
  default: z.unknown().optional(),
});

export type SkillParameter = z.infer<typeof SkillParameterSchema>;

export const SkillSchema = z.object({
  id: z.string(),
  name: z.string().min(1).max(255),
  description: z.string().max(1000),
  version: z.string().regex(/^\d+\.\d+\.\d+$/),
  category: z.string().default('general'),
  parameters: z.array(SkillParameterSchema).default([]),
  examples: z.array(z.object({
    input: z.string(),
    output: z.string(),
  })).default([]),
  isBuiltin: z.boolean().default(false),
  isEnabled: z.boolean().default(true),
  metadata: z.record(z.unknown()).default({}),
});

export type Skill = z.infer<typeof SkillSchema>;

export interface SkillExecutor {
  execute(params: Record<string, unknown>, context: SkillContext): Promise<SkillResult>;
}

export interface SkillContext {
  tenantId: string;
  agentId: string;
  sessionId: string;
  userId?: string;
  memory: MemoryEntry[];
}

export interface SkillResult {
  success: boolean;
  output: unknown;
  error?: string;
  metadata?: Record<string, unknown>;
}

// ============================================================================
// Events
// ============================================================================

export const DomainEventSchema = z.object({
  id: z.string().uuid(),
  type: z.string(),
  tenantId: z.string().uuid(),
  aggregateId: z.string().uuid(),
  aggregateType: z.enum(['agent', 'session', 'memory', 'skill', 'task']),
  payload: z.record(z.unknown()),
  metadata: z.record(z.unknown()).default({}),
  timestamp: z.date(),
});

export type DomainEvent = z.infer<typeof DomainEventSchema>;

// ============================================================================
// Task (Background Workers)
// ============================================================================

export const TaskStatusSchema = z.enum([
  'pending',
  'queued',
  'running',
  'completed',
  'failed',
  'cancelled',
]);

export type TaskStatus = z.infer<typeof TaskStatusSchema>;

export const TaskSchema = z.object({
  id: z.string().uuid(),
  tenantId: z.string().uuid(),
  type: z.string(),
  priority: z.number().min(0).max(100).default(50),
  status: TaskStatusSchema.default('pending'),
  payload: z.record(z.unknown()),
  result: z.unknown().optional(),
  error: z.string().optional(),
  attempts: z.number().default(0),
  maxAttempts: z.number().default(3),
  scheduledAt: z.date().optional(),
  startedAt: z.date().optional(),
  completedAt: z.date().optional(),
  createdAt: z.date(),
  updatedAt: z.date(),
});

export type Task = z.infer<typeof TaskSchema>;

// ============================================================================
// Learning
// ============================================================================

export const TrajectoryStepSchema = z.object({
  action: z.string(),
  input: z.unknown(),
  output: z.unknown(),
  confidence: z.number().min(0).max(1),
  duration: z.number(),
  timestamp: z.date(),
});

export type TrajectoryStep = z.infer<typeof TrajectoryStepSchema>;

export const TrajectorySchema = z.object({
  id: z.string().uuid(),
  tenantId: z.string().uuid(),
  agentId: z.string().uuid(),
  sessionId: z.string().uuid(),
  steps: z.array(TrajectoryStepSchema),
  outcome: z.enum(['success', 'failure', 'partial', 'unknown']),
  reward: z.number().min(-1).max(1).default(0),
  metadata: z.record(z.unknown()).default({}),
  createdAt: z.date(),
});

export type Trajectory = z.infer<typeof TrajectorySchema>;
