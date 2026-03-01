/**
 * Core type definitions for RuvBot
 */
declare const brand: unique symbol;
type Brand<T, B> = T & {
    readonly [brand]: B;
};
export type TenantId = Brand<string, 'TenantId'>;
export type WorkspaceId = Brand<string, 'WorkspaceId'>;
export type UserId = Brand<string, 'UserId'>;
export type AgentId = Brand<string, 'AgentId'>;
export type SessionId = Brand<string, 'SessionId'>;
export type TurnId = Brand<string, 'TurnId'>;
export type MemoryId = Brand<string, 'MemoryId'>;
export type SkillId = Brand<string, 'SkillId'>;
export type PatternId = Brand<string, 'PatternId'>;
export type TrajectoryId = Brand<string, 'TrajectoryId'>;
export interface TenantContext {
    orgId: TenantId;
    workspaceId: WorkspaceId;
    userId: UserId;
    roles: Role[];
    permissions: string[];
}
export type Role = 'org:owner' | 'org:admin' | 'workspace:admin' | 'member' | 'viewer' | 'api_key';
export interface GeoLocation {
    latitude: number;
    longitude: number;
    altitude?: number;
}
export interface TimeRange {
    start: Date;
    end: Date;
}
export interface SemanticVersion {
    major: number;
    minor: number;
    patch: number;
    prerelease?: string;
}
export type Result<T, E = Error> = {
    ok: true;
    value: T;
} | {
    ok: false;
    error: E;
};
export declare function ok<T>(value: T): Result<T, never>;
export declare function err<E>(error: E): Result<never, E>;
export interface DomainEvent<T = unknown> {
    id: string;
    type: string;
    timestamp: Date;
    tenantId: TenantId;
    payload: T;
    metadata?: Record<string, unknown>;
}
export type EventHandler<T extends DomainEvent> = (event: T) => Promise<void>;
export interface RuvBotConfig {
    database: DatabaseConfig;
    redis: RedisConfig;
    vectorStore: VectorStoreConfig;
    llm: LLMConfig;
    slack?: SlackConfig;
    webhooks?: WebhooksConfig;
    learning: LearningConfig;
}
export interface DatabaseConfig {
    host: string;
    port: number;
    database: string;
    user: string;
    password: string;
    ssl?: boolean;
    poolSize?: number;
}
export interface RedisConfig {
    host: string;
    port: number;
    password?: string;
    db?: number;
    tls?: boolean;
}
export interface VectorStoreConfig {
    backend: 'native' | 'wasm' | 'auto';
    dimensions: number;
    hnsw: {
        m: number;
        efConstruction: number;
        efSearch: number;
    };
}
export interface LLMConfig {
    provider: 'anthropic' | 'openai' | 'custom';
    apiKey: string;
    model: string;
    maxTokens?: number;
    temperature?: number;
}
export interface SlackConfig {
    botToken: string;
    signingSecret: string;
    appToken?: string;
}
export interface WebhooksConfig {
    inboundSecret: string;
    outboundRetries: number;
    outboundTimeout: number;
}
export interface LearningConfig {
    enabled: boolean;
    trajectoryCollection: boolean;
    patternMatching: boolean;
    loraTraining: boolean;
    ewcConsolidation: boolean;
    minTrajectoriesForTraining: number;
    trainingIntervalHours: number;
}
export {};
//# sourceMappingURL=types.d.ts.map