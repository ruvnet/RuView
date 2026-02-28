/**
 * Core type definitions for RuvBot
 */
import type { z } from 'zod';
export interface AgentConfig {
    id: string;
    name: string;
    model?: string;
    systemPrompt?: string;
    temperature?: number;
    maxTokens?: number;
    skills?: string[];
    memory?: MemoryConfig;
}
export interface Agent {
    id: string;
    name: string;
    config: AgentConfig;
    status: AgentStatus;
    createdAt: Date;
    lastActiveAt: Date;
}
export type AgentStatus = 'idle' | 'processing' | 'learning' | 'error' | 'stopped';
export interface SessionConfig {
    id?: string;
    agentId: string;
    userId?: string;
    channelId?: string;
    platform?: Platform;
    metadata?: Record<string, unknown>;
    ttl?: number;
}
export interface Session {
    id: string;
    agentId: string;
    userId?: string;
    channelId?: string;
    platform: Platform;
    messages: Message[];
    context: SessionContext;
    metadata: Record<string, unknown>;
    createdAt: Date;
    updatedAt: Date;
    expiresAt?: Date;
}
export interface SessionContext {
    summary?: string;
    topics: string[];
    entities: Entity[];
    embeddings?: Float32Array;
    relevantMemories?: MemoryEntry[];
}
export type Platform = 'slack' | 'discord' | 'web' | 'api' | 'cli' | 'webhook';
export interface Message {
    id: string;
    sessionId: string;
    role: MessageRole;
    content: string;
    attachments?: Attachment[];
    embedding?: Float32Array;
    metadata?: MessageMetadata;
    createdAt: Date;
}
export type MessageRole = 'user' | 'assistant' | 'system' | 'function';
export interface Attachment {
    type: AttachmentType;
    url?: string;
    data?: Buffer;
    mimeType: string;
    name?: string;
    size?: number;
}
export type AttachmentType = 'image' | 'file' | 'code' | 'link';
export interface MessageMetadata {
    tokens?: number;
    latency?: number;
    model?: string;
    skillUsed?: string;
    confidence?: number;
}
export interface MemoryConfig {
    dimensions: number;
    maxVectors: number;
    indexType: 'hnsw' | 'flat' | 'ivf';
    persistPath?: string;
    efConstruction?: number;
    efSearch?: number;
    m?: number;
}
export interface MemoryEntry {
    id: string;
    content: string;
    embedding: Float32Array;
    metadata: MemoryMetadata;
    score?: number;
    createdAt: Date;
    accessedAt: Date;
    accessCount: number;
}
export interface MemoryMetadata {
    source: MemorySource;
    sessionId?: string;
    agentId?: string;
    tags?: string[];
    importance?: number;
    ttl?: number;
}
export type MemorySource = 'conversation' | 'learning' | 'skill' | 'external' | 'user';
export interface MemorySearchOptions {
    topK?: number;
    threshold?: number;
    filter?: MemoryFilter;
    includeMetadata?: boolean;
}
export interface MemoryFilter {
    source?: MemorySource[];
    tags?: string[];
    agentId?: string;
    sessionId?: string;
    after?: Date;
    before?: Date;
}
export interface SkillDefinition {
    name: string;
    description: string;
    version: string;
    author?: string;
    inputs: SkillInput[];
    outputs: SkillOutput[];
    examples?: SkillExample[];
    config?: Record<string, unknown>;
}
export interface SkillInput {
    name: string;
    type: SkillParamType;
    description: string;
    required?: boolean;
    default?: unknown;
    validation?: z.ZodSchema;
}
export interface SkillOutput {
    name: string;
    type: SkillParamType;
    description: string;
}
export type SkillParamType = 'string' | 'number' | 'boolean' | 'array' | 'object' | 'file';
export interface SkillExample {
    input: Record<string, unknown>;
    output: unknown;
    description?: string;
}
export interface SkillContext {
    session: Session;
    agent: Agent;
    memory: MemoryManager;
    llm: LLMOrchestrator;
    emit: (event: string, data: unknown) => void;
}
export interface SkillResult<T = unknown> {
    success: boolean;
    data?: T;
    error?: string;
    metadata?: {
        latency: number;
        tokensUsed?: number;
    };
}
export interface LLMConfig {
    provider: LLMProvider;
    model: string;
    apiKey?: string;
    baseUrl?: string;
    temperature?: number;
    maxTokens?: number;
    streaming?: boolean;
}
export type LLMProvider = 'anthropic' | 'openai' | 'google' | 'local' | 'ruvllm';
export interface LLMRequest {
    messages: LLMMessage[];
    systemPrompt?: string;
    temperature?: number;
    maxTokens?: number;
    tools?: LLMTool[];
    stream?: boolean;
}
export interface LLMMessage {
    role: 'user' | 'assistant' | 'system';
    content: string;
}
export interface LLMTool {
    name: string;
    description: string;
    parameters: Record<string, unknown>;
}
export interface LLMResponse {
    content: string;
    model: string;
    usage: {
        promptTokens: number;
        completionTokens: number;
        totalTokens: number;
    };
    finishReason: 'stop' | 'length' | 'tool_use' | 'error';
    toolCalls?: LLMToolCall[];
}
export interface LLMToolCall {
    id: string;
    name: string;
    arguments: Record<string, unknown>;
}
export interface BotEvent<T = unknown> {
    type: BotEventType;
    timestamp: Date;
    source: string;
    data: T;
}
export type BotEventType = 'message:received' | 'message:sent' | 'session:created' | 'session:ended' | 'skill:invoked' | 'skill:completed' | 'memory:stored' | 'memory:retrieved' | 'learning:pattern' | 'error:occurred' | 'agent:status';
export interface Entity {
    type: EntityType;
    value: string;
    confidence: number;
    start?: number;
    end?: number;
}
export type EntityType = 'person' | 'organization' | 'location' | 'date' | 'time' | 'money' | 'percent' | 'code' | 'url' | 'email' | 'custom';
export type Result<T, E = Error> = Ok<T> | Err<E>;
export interface Ok<T> {
    ok: true;
    value: T;
}
export interface Err<E> {
    ok: false;
    error: E;
}
export declare function ok<T>(value: T): Ok<T>;
export declare function err<E>(error: E): Err<E>;
export interface MemoryManager {
    search(query: string, options?: MemorySearchOptions): Promise<MemoryEntry[]>;
    store(content: string, metadata?: Partial<MemoryMetadata>): Promise<MemoryEntry>;
    delete(id: string): Promise<boolean>;
    getById(id: string): Promise<MemoryEntry | null>;
}
export interface LLMOrchestrator {
    complete(request: LLMRequest): Promise<LLMResponse>;
    stream(request: LLMRequest): AsyncIterable<string>;
    embed(text: string): Promise<Float32Array>;
}
//# sourceMappingURL=types.d.ts.map