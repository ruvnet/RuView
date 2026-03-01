/**
 * Bot configuration management
 */
import { z } from 'zod';
export declare const MemoryConfigSchema: z.ZodObject<{
    dimensions: z.ZodDefault<z.ZodNumber>;
    maxVectors: z.ZodDefault<z.ZodNumber>;
    indexType: z.ZodDefault<z.ZodEnum<["hnsw", "flat", "ivf"]>>;
    persistPath: z.ZodOptional<z.ZodString>;
    efConstruction: z.ZodDefault<z.ZodNumber>;
    efSearch: z.ZodDefault<z.ZodNumber>;
    m: z.ZodDefault<z.ZodNumber>;
}, "strip", z.ZodTypeAny, {
    m: number;
    dimensions: number;
    maxVectors: number;
    indexType: "flat" | "hnsw" | "ivf";
    efConstruction: number;
    efSearch: number;
    persistPath?: string | undefined;
}, {
    m?: number | undefined;
    dimensions?: number | undefined;
    maxVectors?: number | undefined;
    indexType?: "flat" | "hnsw" | "ivf" | undefined;
    persistPath?: string | undefined;
    efConstruction?: number | undefined;
    efSearch?: number | undefined;
}>;
export declare const LLMConfigSchema: z.ZodObject<{
    provider: z.ZodDefault<z.ZodEnum<["anthropic", "openai", "google", "local", "ruvllm"]>>;
    model: z.ZodDefault<z.ZodString>;
    apiKey: z.ZodOptional<z.ZodString>;
    baseUrl: z.ZodOptional<z.ZodString>;
    temperature: z.ZodDefault<z.ZodNumber>;
    maxTokens: z.ZodDefault<z.ZodNumber>;
    streaming: z.ZodDefault<z.ZodBoolean>;
}, "strip", z.ZodTypeAny, {
    provider: "anthropic" | "openai" | "local" | "google" | "ruvllm";
    model: string;
    streaming: boolean;
    temperature: number;
    maxTokens: number;
    apiKey?: string | undefined;
    baseUrl?: string | undefined;
}, {
    provider?: "anthropic" | "openai" | "local" | "google" | "ruvllm" | undefined;
    apiKey?: string | undefined;
    model?: string | undefined;
    streaming?: boolean | undefined;
    temperature?: number | undefined;
    maxTokens?: number | undefined;
    baseUrl?: string | undefined;
}>;
export declare const SlackConfigSchema: z.ZodObject<{
    enabled: z.ZodDefault<z.ZodBoolean>;
    botToken: z.ZodOptional<z.ZodString>;
    signingSecret: z.ZodOptional<z.ZodString>;
    appToken: z.ZodOptional<z.ZodString>;
    socketMode: z.ZodDefault<z.ZodBoolean>;
}, "strip", z.ZodTypeAny, {
    enabled: boolean;
    socketMode: boolean;
    botToken?: string | undefined;
    signingSecret?: string | undefined;
    appToken?: string | undefined;
}, {
    enabled?: boolean | undefined;
    botToken?: string | undefined;
    signingSecret?: string | undefined;
    appToken?: string | undefined;
    socketMode?: boolean | undefined;
}>;
export declare const DiscordConfigSchema: z.ZodObject<{
    enabled: z.ZodDefault<z.ZodBoolean>;
    token: z.ZodOptional<z.ZodString>;
    clientId: z.ZodOptional<z.ZodString>;
    guildId: z.ZodOptional<z.ZodString>;
}, "strip", z.ZodTypeAny, {
    enabled: boolean;
    token?: string | undefined;
    clientId?: string | undefined;
    guildId?: string | undefined;
}, {
    token?: string | undefined;
    enabled?: boolean | undefined;
    clientId?: string | undefined;
    guildId?: string | undefined;
}>;
export declare const WebhookConfigSchema: z.ZodObject<{
    enabled: z.ZodDefault<z.ZodBoolean>;
    secret: z.ZodOptional<z.ZodString>;
    endpoints: z.ZodDefault<z.ZodArray<z.ZodString, "many">>;
}, "strip", z.ZodTypeAny, {
    endpoints: string[];
    enabled: boolean;
    secret?: string | undefined;
}, {
    endpoints?: string[] | undefined;
    enabled?: boolean | undefined;
    secret?: string | undefined;
}>;
export declare const APIConfigSchema: z.ZodObject<{
    enabled: z.ZodDefault<z.ZodBoolean>;
    port: z.ZodDefault<z.ZodNumber>;
    host: z.ZodDefault<z.ZodString>;
    cors: z.ZodDefault<z.ZodBoolean>;
    rateLimit: z.ZodDefault<z.ZodObject<{
        max: z.ZodDefault<z.ZodNumber>;
        timeWindow: z.ZodDefault<z.ZodNumber>;
    }, "strip", z.ZodTypeAny, {
        timeWindow: number;
        max: number;
    }, {
        timeWindow?: number | undefined;
        max?: number | undefined;
    }>>;
    auth: z.ZodDefault<z.ZodObject<{
        enabled: z.ZodDefault<z.ZodBoolean>;
        type: z.ZodDefault<z.ZodEnum<["bearer", "basic", "apikey"]>>;
        secret: z.ZodOptional<z.ZodString>;
    }, "strip", z.ZodTypeAny, {
        type: "basic" | "bearer" | "apikey";
        enabled: boolean;
        secret?: string | undefined;
    }, {
        type?: "basic" | "bearer" | "apikey" | undefined;
        enabled?: boolean | undefined;
        secret?: string | undefined;
    }>>;
}, "strip", z.ZodTypeAny, {
    port: number;
    enabled: boolean;
    auth: {
        type: "basic" | "bearer" | "apikey";
        enabled: boolean;
        secret?: string | undefined;
    };
    host: string;
    rateLimit: {
        timeWindow: number;
        max: number;
    };
    cors: boolean;
}, {
    port?: number | undefined;
    enabled?: boolean | undefined;
    auth?: {
        type?: "basic" | "bearer" | "apikey" | undefined;
        enabled?: boolean | undefined;
        secret?: string | undefined;
    } | undefined;
    host?: string | undefined;
    rateLimit?: {
        timeWindow?: number | undefined;
        max?: number | undefined;
    } | undefined;
    cors?: boolean | undefined;
}>;
export declare const StorageConfigSchema: z.ZodObject<{
    type: z.ZodDefault<z.ZodEnum<["sqlite", "postgres", "memory"]>>;
    path: z.ZodDefault<z.ZodString>;
    connectionString: z.ZodOptional<z.ZodString>;
    poolSize: z.ZodDefault<z.ZodNumber>;
}, "strip", z.ZodTypeAny, {
    type: "memory" | "postgres" | "sqlite";
    path: string;
    poolSize: number;
    connectionString?: string | undefined;
}, {
    type?: "memory" | "postgres" | "sqlite" | undefined;
    path?: string | undefined;
    connectionString?: string | undefined;
    poolSize?: number | undefined;
}>;
export declare const LoggingConfigSchema: z.ZodObject<{
    level: z.ZodDefault<z.ZodEnum<["trace", "debug", "info", "warn", "error", "fatal"]>>;
    pretty: z.ZodDefault<z.ZodBoolean>;
    file: z.ZodOptional<z.ZodString>;
}, "strip", z.ZodTypeAny, {
    level: "error" | "debug" | "info" | "warn" | "trace" | "fatal";
    pretty: boolean;
    file?: string | undefined;
}, {
    level?: "error" | "debug" | "info" | "warn" | "trace" | "fatal" | undefined;
    file?: string | undefined;
    pretty?: boolean | undefined;
}>;
export declare const BotConfigSchema: z.ZodObject<{
    name: z.ZodDefault<z.ZodString>;
    version: z.ZodDefault<z.ZodString>;
    environment: z.ZodDefault<z.ZodEnum<["development", "staging", "production"]>>;
    memory: z.ZodDefault<z.ZodObject<{
        dimensions: z.ZodDefault<z.ZodNumber>;
        maxVectors: z.ZodDefault<z.ZodNumber>;
        indexType: z.ZodDefault<z.ZodEnum<["hnsw", "flat", "ivf"]>>;
        persistPath: z.ZodOptional<z.ZodString>;
        efConstruction: z.ZodDefault<z.ZodNumber>;
        efSearch: z.ZodDefault<z.ZodNumber>;
        m: z.ZodDefault<z.ZodNumber>;
    }, "strip", z.ZodTypeAny, {
        m: number;
        dimensions: number;
        maxVectors: number;
        indexType: "flat" | "hnsw" | "ivf";
        efConstruction: number;
        efSearch: number;
        persistPath?: string | undefined;
    }, {
        m?: number | undefined;
        dimensions?: number | undefined;
        maxVectors?: number | undefined;
        indexType?: "flat" | "hnsw" | "ivf" | undefined;
        persistPath?: string | undefined;
        efConstruction?: number | undefined;
        efSearch?: number | undefined;
    }>>;
    llm: z.ZodDefault<z.ZodObject<{
        provider: z.ZodDefault<z.ZodEnum<["anthropic", "openai", "google", "local", "ruvllm"]>>;
        model: z.ZodDefault<z.ZodString>;
        apiKey: z.ZodOptional<z.ZodString>;
        baseUrl: z.ZodOptional<z.ZodString>;
        temperature: z.ZodDefault<z.ZodNumber>;
        maxTokens: z.ZodDefault<z.ZodNumber>;
        streaming: z.ZodDefault<z.ZodBoolean>;
    }, "strip", z.ZodTypeAny, {
        provider: "anthropic" | "openai" | "local" | "google" | "ruvllm";
        model: string;
        streaming: boolean;
        temperature: number;
        maxTokens: number;
        apiKey?: string | undefined;
        baseUrl?: string | undefined;
    }, {
        provider?: "anthropic" | "openai" | "local" | "google" | "ruvllm" | undefined;
        apiKey?: string | undefined;
        model?: string | undefined;
        streaming?: boolean | undefined;
        temperature?: number | undefined;
        maxTokens?: number | undefined;
        baseUrl?: string | undefined;
    }>>;
    storage: z.ZodDefault<z.ZodObject<{
        type: z.ZodDefault<z.ZodEnum<["sqlite", "postgres", "memory"]>>;
        path: z.ZodDefault<z.ZodString>;
        connectionString: z.ZodOptional<z.ZodString>;
        poolSize: z.ZodDefault<z.ZodNumber>;
    }, "strip", z.ZodTypeAny, {
        type: "memory" | "postgres" | "sqlite";
        path: string;
        poolSize: number;
        connectionString?: string | undefined;
    }, {
        type?: "memory" | "postgres" | "sqlite" | undefined;
        path?: string | undefined;
        connectionString?: string | undefined;
        poolSize?: number | undefined;
    }>>;
    logging: z.ZodDefault<z.ZodObject<{
        level: z.ZodDefault<z.ZodEnum<["trace", "debug", "info", "warn", "error", "fatal"]>>;
        pretty: z.ZodDefault<z.ZodBoolean>;
        file: z.ZodOptional<z.ZodString>;
    }, "strip", z.ZodTypeAny, {
        level: "error" | "debug" | "info" | "warn" | "trace" | "fatal";
        pretty: boolean;
        file?: string | undefined;
    }, {
        level?: "error" | "debug" | "info" | "warn" | "trace" | "fatal" | undefined;
        file?: string | undefined;
        pretty?: boolean | undefined;
    }>>;
    api: z.ZodDefault<z.ZodObject<{
        enabled: z.ZodDefault<z.ZodBoolean>;
        port: z.ZodDefault<z.ZodNumber>;
        host: z.ZodDefault<z.ZodString>;
        cors: z.ZodDefault<z.ZodBoolean>;
        rateLimit: z.ZodDefault<z.ZodObject<{
            max: z.ZodDefault<z.ZodNumber>;
            timeWindow: z.ZodDefault<z.ZodNumber>;
        }, "strip", z.ZodTypeAny, {
            timeWindow: number;
            max: number;
        }, {
            timeWindow?: number | undefined;
            max?: number | undefined;
        }>>;
        auth: z.ZodDefault<z.ZodObject<{
            enabled: z.ZodDefault<z.ZodBoolean>;
            type: z.ZodDefault<z.ZodEnum<["bearer", "basic", "apikey"]>>;
            secret: z.ZodOptional<z.ZodString>;
        }, "strip", z.ZodTypeAny, {
            type: "basic" | "bearer" | "apikey";
            enabled: boolean;
            secret?: string | undefined;
        }, {
            type?: "basic" | "bearer" | "apikey" | undefined;
            enabled?: boolean | undefined;
            secret?: string | undefined;
        }>>;
    }, "strip", z.ZodTypeAny, {
        port: number;
        enabled: boolean;
        auth: {
            type: "basic" | "bearer" | "apikey";
            enabled: boolean;
            secret?: string | undefined;
        };
        host: string;
        rateLimit: {
            timeWindow: number;
            max: number;
        };
        cors: boolean;
    }, {
        port?: number | undefined;
        enabled?: boolean | undefined;
        auth?: {
            type?: "basic" | "bearer" | "apikey" | undefined;
            enabled?: boolean | undefined;
            secret?: string | undefined;
        } | undefined;
        host?: string | undefined;
        rateLimit?: {
            timeWindow?: number | undefined;
            max?: number | undefined;
        } | undefined;
        cors?: boolean | undefined;
    }>>;
    slack: z.ZodDefault<z.ZodObject<{
        enabled: z.ZodDefault<z.ZodBoolean>;
        botToken: z.ZodOptional<z.ZodString>;
        signingSecret: z.ZodOptional<z.ZodString>;
        appToken: z.ZodOptional<z.ZodString>;
        socketMode: z.ZodDefault<z.ZodBoolean>;
    }, "strip", z.ZodTypeAny, {
        enabled: boolean;
        socketMode: boolean;
        botToken?: string | undefined;
        signingSecret?: string | undefined;
        appToken?: string | undefined;
    }, {
        enabled?: boolean | undefined;
        botToken?: string | undefined;
        signingSecret?: string | undefined;
        appToken?: string | undefined;
        socketMode?: boolean | undefined;
    }>>;
    discord: z.ZodDefault<z.ZodObject<{
        enabled: z.ZodDefault<z.ZodBoolean>;
        token: z.ZodOptional<z.ZodString>;
        clientId: z.ZodOptional<z.ZodString>;
        guildId: z.ZodOptional<z.ZodString>;
    }, "strip", z.ZodTypeAny, {
        enabled: boolean;
        token?: string | undefined;
        clientId?: string | undefined;
        guildId?: string | undefined;
    }, {
        token?: string | undefined;
        enabled?: boolean | undefined;
        clientId?: string | undefined;
        guildId?: string | undefined;
    }>>;
    webhook: z.ZodDefault<z.ZodObject<{
        enabled: z.ZodDefault<z.ZodBoolean>;
        secret: z.ZodOptional<z.ZodString>;
        endpoints: z.ZodDefault<z.ZodArray<z.ZodString, "many">>;
    }, "strip", z.ZodTypeAny, {
        endpoints: string[];
        enabled: boolean;
        secret?: string | undefined;
    }, {
        endpoints?: string[] | undefined;
        enabled?: boolean | undefined;
        secret?: string | undefined;
    }>>;
    skills: z.ZodDefault<z.ZodObject<{
        enabled: z.ZodDefault<z.ZodArray<z.ZodString, "many">>;
        custom: z.ZodDefault<z.ZodArray<z.ZodString, "many">>;
        directory: z.ZodDefault<z.ZodString>;
    }, "strip", z.ZodTypeAny, {
        custom: string[];
        enabled: string[];
        directory: string;
    }, {
        custom?: string[] | undefined;
        enabled?: string[] | undefined;
        directory?: string | undefined;
    }>>;
    session: z.ZodDefault<z.ZodObject<{
        defaultTTL: z.ZodDefault<z.ZodNumber>;
        maxPerUser: z.ZodDefault<z.ZodNumber>;
        maxMessages: z.ZodDefault<z.ZodNumber>;
    }, "strip", z.ZodTypeAny, {
        defaultTTL: number;
        maxPerUser: number;
        maxMessages: number;
    }, {
        defaultTTL?: number | undefined;
        maxPerUser?: number | undefined;
        maxMessages?: number | undefined;
    }>>;
    workers: z.ZodDefault<z.ZodObject<{
        poolSize: z.ZodDefault<z.ZodNumber>;
        taskTimeout: z.ZodDefault<z.ZodNumber>;
        retryAttempts: z.ZodDefault<z.ZodNumber>;
    }, "strip", z.ZodTypeAny, {
        retryAttempts: number;
        poolSize: number;
        taskTimeout: number;
    }, {
        retryAttempts?: number | undefined;
        poolSize?: number | undefined;
        taskTimeout?: number | undefined;
    }>>;
}, "strip", z.ZodTypeAny, {
    memory: {
        m: number;
        dimensions: number;
        maxVectors: number;
        indexType: "flat" | "hnsw" | "ivf";
        efConstruction: number;
        efSearch: number;
        persistPath?: string | undefined;
    };
    name: string;
    version: string;
    skills: {
        custom: string[];
        enabled: string[];
        directory: string;
    };
    environment: "development" | "staging" | "production";
    api: {
        port: number;
        enabled: boolean;
        auth: {
            type: "basic" | "bearer" | "apikey";
            enabled: boolean;
            secret?: string | undefined;
        };
        host: string;
        rateLimit: {
            timeWindow: number;
            max: number;
        };
        cors: boolean;
    };
    workers: {
        retryAttempts: number;
        poolSize: number;
        taskTimeout: number;
    };
    storage: {
        type: "memory" | "postgres" | "sqlite";
        path: string;
        poolSize: number;
        connectionString?: string | undefined;
    };
    llm: {
        provider: "anthropic" | "openai" | "local" | "google" | "ruvllm";
        model: string;
        streaming: boolean;
        temperature: number;
        maxTokens: number;
        apiKey?: string | undefined;
        baseUrl?: string | undefined;
    };
    slack: {
        enabled: boolean;
        socketMode: boolean;
        botToken?: string | undefined;
        signingSecret?: string | undefined;
        appToken?: string | undefined;
    };
    discord: {
        enabled: boolean;
        token?: string | undefined;
        clientId?: string | undefined;
        guildId?: string | undefined;
    };
    webhook: {
        endpoints: string[];
        enabled: boolean;
        secret?: string | undefined;
    };
    logging: {
        level: "error" | "debug" | "info" | "warn" | "trace" | "fatal";
        pretty: boolean;
        file?: string | undefined;
    };
    session: {
        defaultTTL: number;
        maxPerUser: number;
        maxMessages: number;
    };
}, {
    memory?: {
        m?: number | undefined;
        dimensions?: number | undefined;
        maxVectors?: number | undefined;
        indexType?: "flat" | "hnsw" | "ivf" | undefined;
        persistPath?: string | undefined;
        efConstruction?: number | undefined;
        efSearch?: number | undefined;
    } | undefined;
    name?: string | undefined;
    version?: string | undefined;
    skills?: {
        custom?: string[] | undefined;
        enabled?: string[] | undefined;
        directory?: string | undefined;
    } | undefined;
    environment?: "development" | "staging" | "production" | undefined;
    api?: {
        port?: number | undefined;
        enabled?: boolean | undefined;
        auth?: {
            type?: "basic" | "bearer" | "apikey" | undefined;
            enabled?: boolean | undefined;
            secret?: string | undefined;
        } | undefined;
        host?: string | undefined;
        rateLimit?: {
            timeWindow?: number | undefined;
            max?: number | undefined;
        } | undefined;
        cors?: boolean | undefined;
    } | undefined;
    workers?: {
        retryAttempts?: number | undefined;
        poolSize?: number | undefined;
        taskTimeout?: number | undefined;
    } | undefined;
    storage?: {
        type?: "memory" | "postgres" | "sqlite" | undefined;
        path?: string | undefined;
        connectionString?: string | undefined;
        poolSize?: number | undefined;
    } | undefined;
    llm?: {
        provider?: "anthropic" | "openai" | "local" | "google" | "ruvllm" | undefined;
        apiKey?: string | undefined;
        model?: string | undefined;
        streaming?: boolean | undefined;
        temperature?: number | undefined;
        maxTokens?: number | undefined;
        baseUrl?: string | undefined;
    } | undefined;
    slack?: {
        enabled?: boolean | undefined;
        botToken?: string | undefined;
        signingSecret?: string | undefined;
        appToken?: string | undefined;
        socketMode?: boolean | undefined;
    } | undefined;
    discord?: {
        token?: string | undefined;
        enabled?: boolean | undefined;
        clientId?: string | undefined;
        guildId?: string | undefined;
    } | undefined;
    webhook?: {
        endpoints?: string[] | undefined;
        enabled?: boolean | undefined;
        secret?: string | undefined;
    } | undefined;
    logging?: {
        level?: "error" | "debug" | "info" | "warn" | "trace" | "fatal" | undefined;
        file?: string | undefined;
        pretty?: boolean | undefined;
    } | undefined;
    session?: {
        defaultTTL?: number | undefined;
        maxPerUser?: number | undefined;
        maxMessages?: number | undefined;
    } | undefined;
}>;
export type BotConfig = z.infer<typeof BotConfigSchema>;
export declare class ConfigManager {
    private config;
    constructor(initialConfig?: Partial<BotConfig>);
    /**
     * Get the full configuration
     */
    getConfig(): Readonly<BotConfig>;
    /**
     * Get a specific configuration section
     */
    get<K extends keyof BotConfig>(key: K): BotConfig[K];
    /**
     * Update configuration
     */
    update(updates: Partial<BotConfig>): void;
    /**
     * Validate configuration
     */
    validate(): {
        valid: boolean;
        errors: string[];
    };
    /**
     * Load configuration from environment variables
     */
    static fromEnv(): ConfigManager;
    /**
     * Export configuration as JSON
     */
    toJSON(): string;
}
//# sourceMappingURL=BotConfig.d.ts.map