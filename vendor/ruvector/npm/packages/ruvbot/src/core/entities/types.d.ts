/**
 * Core type definitions for RuvBot entities
 */
import { z } from 'zod';
export declare const TenantSchema: z.ZodObject<{
    id: z.ZodString;
    name: z.ZodString;
    slug: z.ZodString;
    settings: z.ZodDefault<z.ZodRecord<z.ZodString, z.ZodUnknown>>;
    createdAt: z.ZodDate;
    updatedAt: z.ZodDate;
}, "strip", z.ZodTypeAny, {
    id: string;
    createdAt: Date;
    name: string;
    slug: string;
    updatedAt: Date;
    settings: Record<string, unknown>;
}, {
    id: string;
    createdAt: Date;
    name: string;
    slug: string;
    updatedAt: Date;
    settings?: Record<string, unknown> | undefined;
}>;
export type Tenant = z.infer<typeof TenantSchema>;
export declare const AgentStatusSchema: z.ZodEnum<["idle", "processing", "learning", "error", "offline"]>;
export type AgentStatus = z.infer<typeof AgentStatusSchema>;
export declare const AgentConfigSchema: z.ZodObject<{
    modelProvider: z.ZodDefault<z.ZodEnum<["anthropic", "openai", "local"]>>;
    modelId: z.ZodDefault<z.ZodString>;
    temperature: z.ZodDefault<z.ZodNumber>;
    maxTokens: z.ZodDefault<z.ZodNumber>;
    systemPrompt: z.ZodOptional<z.ZodString>;
    skills: z.ZodDefault<z.ZodArray<z.ZodString, "many">>;
    learningEnabled: z.ZodDefault<z.ZodBoolean>;
    memoryEnabled: z.ZodDefault<z.ZodBoolean>;
}, "strip", z.ZodTypeAny, {
    temperature: number;
    maxTokens: number;
    modelId: string;
    skills: string[];
    modelProvider: "anthropic" | "openai" | "local";
    learningEnabled: boolean;
    memoryEnabled: boolean;
    systemPrompt?: string | undefined;
}, {
    temperature?: number | undefined;
    maxTokens?: number | undefined;
    modelId?: string | undefined;
    skills?: string[] | undefined;
    systemPrompt?: string | undefined;
    modelProvider?: "anthropic" | "openai" | "local" | undefined;
    learningEnabled?: boolean | undefined;
    memoryEnabled?: boolean | undefined;
}>;
export type AgentConfig = z.infer<typeof AgentConfigSchema>;
export declare const AgentSchema: z.ZodObject<{
    id: z.ZodString;
    tenantId: z.ZodString;
    name: z.ZodString;
    description: z.ZodOptional<z.ZodString>;
    status: z.ZodDefault<z.ZodEnum<["idle", "processing", "learning", "error", "offline"]>>;
    config: z.ZodObject<{
        modelProvider: z.ZodDefault<z.ZodEnum<["anthropic", "openai", "local"]>>;
        modelId: z.ZodDefault<z.ZodString>;
        temperature: z.ZodDefault<z.ZodNumber>;
        maxTokens: z.ZodDefault<z.ZodNumber>;
        systemPrompt: z.ZodOptional<z.ZodString>;
        skills: z.ZodDefault<z.ZodArray<z.ZodString, "many">>;
        learningEnabled: z.ZodDefault<z.ZodBoolean>;
        memoryEnabled: z.ZodDefault<z.ZodBoolean>;
    }, "strip", z.ZodTypeAny, {
        temperature: number;
        maxTokens: number;
        modelId: string;
        skills: string[];
        modelProvider: "anthropic" | "openai" | "local";
        learningEnabled: boolean;
        memoryEnabled: boolean;
        systemPrompt?: string | undefined;
    }, {
        temperature?: number | undefined;
        maxTokens?: number | undefined;
        modelId?: string | undefined;
        skills?: string[] | undefined;
        systemPrompt?: string | undefined;
        modelProvider?: "anthropic" | "openai" | "local" | undefined;
        learningEnabled?: boolean | undefined;
        memoryEnabled?: boolean | undefined;
    }>;
    metadata: z.ZodDefault<z.ZodRecord<z.ZodString, z.ZodUnknown>>;
    createdAt: z.ZodDate;
    updatedAt: z.ZodDate;
}, "strip", z.ZodTypeAny, {
    id: string;
    createdAt: Date;
    status: "error" | "processing" | "idle" | "learning" | "offline";
    metadata: Record<string, unknown>;
    name: string;
    updatedAt: Date;
    config: {
        temperature: number;
        maxTokens: number;
        modelId: string;
        skills: string[];
        modelProvider: "anthropic" | "openai" | "local";
        learningEnabled: boolean;
        memoryEnabled: boolean;
        systemPrompt?: string | undefined;
    };
    tenantId: string;
    description?: string | undefined;
}, {
    id: string;
    createdAt: Date;
    name: string;
    updatedAt: Date;
    config: {
        temperature?: number | undefined;
        maxTokens?: number | undefined;
        modelId?: string | undefined;
        skills?: string[] | undefined;
        systemPrompt?: string | undefined;
        modelProvider?: "anthropic" | "openai" | "local" | undefined;
        learningEnabled?: boolean | undefined;
        memoryEnabled?: boolean | undefined;
    };
    tenantId: string;
    status?: "error" | "processing" | "idle" | "learning" | "offline" | undefined;
    metadata?: Record<string, unknown> | undefined;
    description?: string | undefined;
}>;
export type Agent = z.infer<typeof AgentSchema>;
export declare const MessageRoleSchema: z.ZodEnum<["user", "assistant", "system", "tool"]>;
export type MessageRole = z.infer<typeof MessageRoleSchema>;
export declare const MessageSchema: z.ZodObject<{
    id: z.ZodString;
    role: z.ZodEnum<["user", "assistant", "system", "tool"]>;
    content: z.ZodString;
    toolCalls: z.ZodOptional<z.ZodArray<z.ZodObject<{
        id: z.ZodString;
        name: z.ZodString;
        arguments: z.ZodRecord<z.ZodString, z.ZodUnknown>;
    }, "strip", z.ZodTypeAny, {
        id: string;
        name: string;
        arguments: Record<string, unknown>;
    }, {
        id: string;
        name: string;
        arguments: Record<string, unknown>;
    }>, "many">>;
    toolResults: z.ZodOptional<z.ZodArray<z.ZodObject<{
        toolCallId: z.ZodString;
        result: z.ZodUnknown;
        isError: z.ZodDefault<z.ZodBoolean>;
    }, "strip", z.ZodTypeAny, {
        toolCallId: string;
        isError: boolean;
        result?: unknown;
    }, {
        toolCallId: string;
        result?: unknown;
        isError?: boolean | undefined;
    }>, "many">>;
    metadata: z.ZodDefault<z.ZodRecord<z.ZodString, z.ZodUnknown>>;
    createdAt: z.ZodDate;
}, "strip", z.ZodTypeAny, {
    id: string;
    createdAt: Date;
    metadata: Record<string, unknown>;
    role: "user" | "system" | "assistant" | "tool";
    content: string;
    toolCalls?: {
        id: string;
        name: string;
        arguments: Record<string, unknown>;
    }[] | undefined;
    toolResults?: {
        toolCallId: string;
        isError: boolean;
        result?: unknown;
    }[] | undefined;
}, {
    id: string;
    createdAt: Date;
    role: "user" | "system" | "assistant" | "tool";
    content: string;
    metadata?: Record<string, unknown> | undefined;
    toolCalls?: {
        id: string;
        name: string;
        arguments: Record<string, unknown>;
    }[] | undefined;
    toolResults?: {
        toolCallId: string;
        result?: unknown;
        isError?: boolean | undefined;
    }[] | undefined;
}>;
export type Message = z.infer<typeof MessageSchema>;
export declare const SessionSchema: z.ZodObject<{
    id: z.ZodString;
    tenantId: z.ZodString;
    agentId: z.ZodString;
    userId: z.ZodOptional<z.ZodString>;
    channelId: z.ZodOptional<z.ZodString>;
    channelType: z.ZodOptional<z.ZodEnum<["slack", "discord", "webhook", "api", "cli"]>>;
    messages: z.ZodDefault<z.ZodArray<z.ZodObject<{
        id: z.ZodString;
        role: z.ZodEnum<["user", "assistant", "system", "tool"]>;
        content: z.ZodString;
        toolCalls: z.ZodOptional<z.ZodArray<z.ZodObject<{
            id: z.ZodString;
            name: z.ZodString;
            arguments: z.ZodRecord<z.ZodString, z.ZodUnknown>;
        }, "strip", z.ZodTypeAny, {
            id: string;
            name: string;
            arguments: Record<string, unknown>;
        }, {
            id: string;
            name: string;
            arguments: Record<string, unknown>;
        }>, "many">>;
        toolResults: z.ZodOptional<z.ZodArray<z.ZodObject<{
            toolCallId: z.ZodString;
            result: z.ZodUnknown;
            isError: z.ZodDefault<z.ZodBoolean>;
        }, "strip", z.ZodTypeAny, {
            toolCallId: string;
            isError: boolean;
            result?: unknown;
        }, {
            toolCallId: string;
            result?: unknown;
            isError?: boolean | undefined;
        }>, "many">>;
        metadata: z.ZodDefault<z.ZodRecord<z.ZodString, z.ZodUnknown>>;
        createdAt: z.ZodDate;
    }, "strip", z.ZodTypeAny, {
        id: string;
        createdAt: Date;
        metadata: Record<string, unknown>;
        role: "user" | "system" | "assistant" | "tool";
        content: string;
        toolCalls?: {
            id: string;
            name: string;
            arguments: Record<string, unknown>;
        }[] | undefined;
        toolResults?: {
            toolCallId: string;
            isError: boolean;
            result?: unknown;
        }[] | undefined;
    }, {
        id: string;
        createdAt: Date;
        role: "user" | "system" | "assistant" | "tool";
        content: string;
        metadata?: Record<string, unknown> | undefined;
        toolCalls?: {
            id: string;
            name: string;
            arguments: Record<string, unknown>;
        }[] | undefined;
        toolResults?: {
            toolCallId: string;
            result?: unknown;
            isError?: boolean | undefined;
        }[] | undefined;
    }>, "many">>;
    context: z.ZodDefault<z.ZodRecord<z.ZodString, z.ZodUnknown>>;
    isActive: z.ZodDefault<z.ZodBoolean>;
    expiresAt: z.ZodOptional<z.ZodDate>;
    createdAt: z.ZodDate;
    updatedAt: z.ZodDate;
}, "strip", z.ZodTypeAny, {
    id: string;
    createdAt: Date;
    agentId: string;
    isActive: boolean;
    updatedAt: Date;
    context: Record<string, unknown>;
    messages: {
        id: string;
        createdAt: Date;
        metadata: Record<string, unknown>;
        role: "user" | "system" | "assistant" | "tool";
        content: string;
        toolCalls?: {
            id: string;
            name: string;
            arguments: Record<string, unknown>;
        }[] | undefined;
        toolResults?: {
            toolCallId: string;
            isError: boolean;
            result?: unknown;
        }[] | undefined;
    }[];
    tenantId: string;
    userId?: string | undefined;
    expiresAt?: Date | undefined;
    channelId?: string | undefined;
    channelType?: "api" | "slack" | "discord" | "cli" | "webhook" | undefined;
}, {
    id: string;
    createdAt: Date;
    agentId: string;
    updatedAt: Date;
    tenantId: string;
    userId?: string | undefined;
    isActive?: boolean | undefined;
    expiresAt?: Date | undefined;
    context?: Record<string, unknown> | undefined;
    messages?: {
        id: string;
        createdAt: Date;
        role: "user" | "system" | "assistant" | "tool";
        content: string;
        metadata?: Record<string, unknown> | undefined;
        toolCalls?: {
            id: string;
            name: string;
            arguments: Record<string, unknown>;
        }[] | undefined;
        toolResults?: {
            toolCallId: string;
            result?: unknown;
            isError?: boolean | undefined;
        }[] | undefined;
    }[] | undefined;
    channelId?: string | undefined;
    channelType?: "api" | "slack" | "discord" | "cli" | "webhook" | undefined;
}>;
export type Session = z.infer<typeof SessionSchema>;
export declare const MemoryTypeSchema: z.ZodEnum<["episodic", "semantic", "procedural", "working"]>;
export type MemoryType = z.infer<typeof MemoryTypeSchema>;
export declare const MemoryEntrySchema: z.ZodObject<{
    id: z.ZodString;
    tenantId: z.ZodString;
    agentId: z.ZodString;
    type: z.ZodEnum<["episodic", "semantic", "procedural", "working"]>;
    content: z.ZodString;
    embedding: z.ZodOptional<z.ZodArray<z.ZodNumber, "many">>;
    importance: z.ZodDefault<z.ZodNumber>;
    accessCount: z.ZodDefault<z.ZodNumber>;
    lastAccessedAt: z.ZodOptional<z.ZodDate>;
    metadata: z.ZodDefault<z.ZodRecord<z.ZodString, z.ZodUnknown>>;
    tags: z.ZodDefault<z.ZodArray<z.ZodString, "many">>;
    expiresAt: z.ZodOptional<z.ZodDate>;
    createdAt: z.ZodDate;
    updatedAt: z.ZodDate;
}, "strip", z.ZodTypeAny, {
    id: string;
    createdAt: Date;
    type: "semantic" | "episodic" | "procedural" | "working";
    agentId: string;
    metadata: Record<string, unknown>;
    content: string;
    tags: string[];
    updatedAt: Date;
    importance: number;
    tenantId: string;
    accessCount: number;
    expiresAt?: Date | undefined;
    embedding?: number[] | undefined;
    lastAccessedAt?: Date | undefined;
}, {
    id: string;
    createdAt: Date;
    type: "semantic" | "episodic" | "procedural" | "working";
    agentId: string;
    content: string;
    updatedAt: Date;
    tenantId: string;
    metadata?: Record<string, unknown> | undefined;
    tags?: string[] | undefined;
    expiresAt?: Date | undefined;
    embedding?: number[] | undefined;
    importance?: number | undefined;
    accessCount?: number | undefined;
    lastAccessedAt?: Date | undefined;
}>;
export type MemoryEntry = z.infer<typeof MemoryEntrySchema>;
export declare const SkillParameterSchema: z.ZodObject<{
    name: z.ZodString;
    type: z.ZodEnum<["string", "number", "boolean", "object", "array"]>;
    description: z.ZodString;
    required: z.ZodDefault<z.ZodBoolean>;
    default: z.ZodOptional<z.ZodUnknown>;
}, "strip", z.ZodTypeAny, {
    type: "string" | "number" | "boolean" | "object" | "array";
    required: boolean;
    name: string;
    description: string;
    default?: unknown;
}, {
    type: "string" | "number" | "boolean" | "object" | "array";
    name: string;
    description: string;
    default?: unknown;
    required?: boolean | undefined;
}>;
export type SkillParameter = z.infer<typeof SkillParameterSchema>;
export declare const SkillSchema: z.ZodObject<{
    id: z.ZodString;
    name: z.ZodString;
    description: z.ZodString;
    version: z.ZodString;
    category: z.ZodDefault<z.ZodString>;
    parameters: z.ZodDefault<z.ZodArray<z.ZodObject<{
        name: z.ZodString;
        type: z.ZodEnum<["string", "number", "boolean", "object", "array"]>;
        description: z.ZodString;
        required: z.ZodDefault<z.ZodBoolean>;
        default: z.ZodOptional<z.ZodUnknown>;
    }, "strip", z.ZodTypeAny, {
        type: "string" | "number" | "boolean" | "object" | "array";
        required: boolean;
        name: string;
        description: string;
        default?: unknown;
    }, {
        type: "string" | "number" | "boolean" | "object" | "array";
        name: string;
        description: string;
        default?: unknown;
        required?: boolean | undefined;
    }>, "many">>;
    examples: z.ZodDefault<z.ZodArray<z.ZodObject<{
        input: z.ZodString;
        output: z.ZodString;
    }, "strip", z.ZodTypeAny, {
        input: string;
        output: string;
    }, {
        input: string;
        output: string;
    }>, "many">>;
    isBuiltin: z.ZodDefault<z.ZodBoolean>;
    isEnabled: z.ZodDefault<z.ZodBoolean>;
    metadata: z.ZodDefault<z.ZodRecord<z.ZodString, z.ZodUnknown>>;
}, "strip", z.ZodTypeAny, {
    id: string;
    metadata: Record<string, unknown>;
    category: string;
    name: string;
    description: string;
    parameters: {
        type: "string" | "number" | "boolean" | "object" | "array";
        required: boolean;
        name: string;
        description: string;
        default?: unknown;
    }[];
    version: string;
    examples: {
        input: string;
        output: string;
    }[];
    isBuiltin: boolean;
    isEnabled: boolean;
}, {
    id: string;
    name: string;
    description: string;
    version: string;
    metadata?: Record<string, unknown> | undefined;
    category?: string | undefined;
    parameters?: {
        type: "string" | "number" | "boolean" | "object" | "array";
        name: string;
        description: string;
        default?: unknown;
        required?: boolean | undefined;
    }[] | undefined;
    examples?: {
        input: string;
        output: string;
    }[] | undefined;
    isBuiltin?: boolean | undefined;
    isEnabled?: boolean | undefined;
}>;
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
export declare const DomainEventSchema: z.ZodObject<{
    id: z.ZodString;
    type: z.ZodString;
    tenantId: z.ZodString;
    aggregateId: z.ZodString;
    aggregateType: z.ZodEnum<["agent", "session", "memory", "skill", "task"]>;
    payload: z.ZodRecord<z.ZodString, z.ZodUnknown>;
    metadata: z.ZodDefault<z.ZodRecord<z.ZodString, z.ZodUnknown>>;
    timestamp: z.ZodDate;
}, "strip", z.ZodTypeAny, {
    id: string;
    type: string;
    payload: Record<string, unknown>;
    metadata: Record<string, unknown>;
    timestamp: Date;
    tenantId: string;
    aggregateId: string;
    aggregateType: "agent" | "memory" | "skill" | "task" | "session";
}, {
    id: string;
    type: string;
    payload: Record<string, unknown>;
    timestamp: Date;
    tenantId: string;
    aggregateId: string;
    aggregateType: "agent" | "memory" | "skill" | "task" | "session";
    metadata?: Record<string, unknown> | undefined;
}>;
export type DomainEvent = z.infer<typeof DomainEventSchema>;
export declare const TaskStatusSchema: z.ZodEnum<["pending", "queued", "running", "completed", "failed", "cancelled"]>;
export type TaskStatus = z.infer<typeof TaskStatusSchema>;
export declare const TaskSchema: z.ZodObject<{
    id: z.ZodString;
    tenantId: z.ZodString;
    type: z.ZodString;
    priority: z.ZodDefault<z.ZodNumber>;
    status: z.ZodDefault<z.ZodEnum<["pending", "queued", "running", "completed", "failed", "cancelled"]>>;
    payload: z.ZodRecord<z.ZodString, z.ZodUnknown>;
    result: z.ZodOptional<z.ZodUnknown>;
    error: z.ZodOptional<z.ZodString>;
    attempts: z.ZodDefault<z.ZodNumber>;
    maxAttempts: z.ZodDefault<z.ZodNumber>;
    scheduledAt: z.ZodOptional<z.ZodDate>;
    startedAt: z.ZodOptional<z.ZodDate>;
    completedAt: z.ZodOptional<z.ZodDate>;
    createdAt: z.ZodDate;
    updatedAt: z.ZodDate;
}, "strip", z.ZodTypeAny, {
    id: string;
    createdAt: Date;
    type: string;
    payload: Record<string, unknown>;
    priority: number;
    status: "pending" | "failed" | "completed" | "running" | "cancelled" | "queued";
    updatedAt: Date;
    tenantId: string;
    attempts: number;
    maxAttempts: number;
    result?: unknown;
    error?: string | undefined;
    startedAt?: Date | undefined;
    completedAt?: Date | undefined;
    scheduledAt?: Date | undefined;
}, {
    id: string;
    createdAt: Date;
    type: string;
    payload: Record<string, unknown>;
    updatedAt: Date;
    tenantId: string;
    priority?: number | undefined;
    status?: "pending" | "failed" | "completed" | "running" | "cancelled" | "queued" | undefined;
    result?: unknown;
    error?: string | undefined;
    startedAt?: Date | undefined;
    completedAt?: Date | undefined;
    attempts?: number | undefined;
    maxAttempts?: number | undefined;
    scheduledAt?: Date | undefined;
}>;
export type Task = z.infer<typeof TaskSchema>;
export declare const TrajectoryStepSchema: z.ZodObject<{
    action: z.ZodString;
    input: z.ZodUnknown;
    output: z.ZodUnknown;
    confidence: z.ZodNumber;
    duration: z.ZodNumber;
    timestamp: z.ZodDate;
}, "strip", z.ZodTypeAny, {
    action: string;
    timestamp: Date;
    duration: number;
    confidence: number;
    input?: unknown;
    output?: unknown;
}, {
    action: string;
    timestamp: Date;
    duration: number;
    confidence: number;
    input?: unknown;
    output?: unknown;
}>;
export type TrajectoryStep = z.infer<typeof TrajectoryStepSchema>;
export declare const TrajectorySchema: z.ZodObject<{
    id: z.ZodString;
    tenantId: z.ZodString;
    agentId: z.ZodString;
    sessionId: z.ZodString;
    steps: z.ZodArray<z.ZodObject<{
        action: z.ZodString;
        input: z.ZodUnknown;
        output: z.ZodUnknown;
        confidence: z.ZodNumber;
        duration: z.ZodNumber;
        timestamp: z.ZodDate;
    }, "strip", z.ZodTypeAny, {
        action: string;
        timestamp: Date;
        duration: number;
        confidence: number;
        input?: unknown;
        output?: unknown;
    }, {
        action: string;
        timestamp: Date;
        duration: number;
        confidence: number;
        input?: unknown;
        output?: unknown;
    }>, "many">;
    outcome: z.ZodEnum<["success", "failure", "partial", "unknown"]>;
    reward: z.ZodDefault<z.ZodNumber>;
    metadata: z.ZodDefault<z.ZodRecord<z.ZodString, z.ZodUnknown>>;
    createdAt: z.ZodDate;
}, "strip", z.ZodTypeAny, {
    id: string;
    createdAt: Date;
    agentId: string;
    metadata: Record<string, unknown>;
    steps: {
        action: string;
        timestamp: Date;
        duration: number;
        confidence: number;
        input?: unknown;
        output?: unknown;
    }[];
    sessionId: string;
    reward: number;
    outcome: "unknown" | "success" | "failure" | "partial";
    tenantId: string;
}, {
    id: string;
    createdAt: Date;
    agentId: string;
    steps: {
        action: string;
        timestamp: Date;
        duration: number;
        confidence: number;
        input?: unknown;
        output?: unknown;
    }[];
    sessionId: string;
    outcome: "unknown" | "success" | "failure" | "partial";
    tenantId: string;
    metadata?: Record<string, unknown> | undefined;
    reward?: number | undefined;
}>;
export type Trajectory = z.infer<typeof TrajectorySchema>;
//# sourceMappingURL=types.d.ts.map