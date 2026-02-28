"use strict";
/**
 * Bot configuration management
 */
Object.defineProperty(exports, "__esModule", { value: true });
exports.ConfigManager = exports.BotConfigSchema = exports.LoggingConfigSchema = exports.StorageConfigSchema = exports.APIConfigSchema = exports.WebhookConfigSchema = exports.DiscordConfigSchema = exports.SlackConfigSchema = exports.LLMConfigSchema = exports.MemoryConfigSchema = void 0;
const zod_1 = require("zod");
// ============================================================================
// Configuration Schema
// ============================================================================
exports.MemoryConfigSchema = zod_1.z.object({
    dimensions: zod_1.z.number().int().min(64).max(4096).default(384),
    maxVectors: zod_1.z.number().int().min(1000).max(10000000).default(100000),
    indexType: zod_1.z.enum(['hnsw', 'flat', 'ivf']).default('hnsw'),
    persistPath: zod_1.z.string().optional(),
    efConstruction: zod_1.z.number().int().min(16).max(500).default(200),
    efSearch: zod_1.z.number().int().min(10).max(500).default(50),
    m: zod_1.z.number().int().min(4).max(64).default(16),
});
exports.LLMConfigSchema = zod_1.z.object({
    provider: zod_1.z.enum(['anthropic', 'openai', 'google', 'local', 'ruvllm']).default('anthropic'),
    model: zod_1.z.string().default('claude-sonnet-4-20250514'),
    apiKey: zod_1.z.string().optional(),
    baseUrl: zod_1.z.string().url().optional(),
    temperature: zod_1.z.number().min(0).max(2).default(0.7),
    maxTokens: zod_1.z.number().int().min(1).max(200000).default(4096),
    streaming: zod_1.z.boolean().default(true),
});
exports.SlackConfigSchema = zod_1.z.object({
    enabled: zod_1.z.boolean().default(false),
    botToken: zod_1.z.string().optional(),
    signingSecret: zod_1.z.string().optional(),
    appToken: zod_1.z.string().optional(),
    socketMode: zod_1.z.boolean().default(true),
});
exports.DiscordConfigSchema = zod_1.z.object({
    enabled: zod_1.z.boolean().default(false),
    token: zod_1.z.string().optional(),
    clientId: zod_1.z.string().optional(),
    guildId: zod_1.z.string().optional(),
});
exports.WebhookConfigSchema = zod_1.z.object({
    enabled: zod_1.z.boolean().default(false),
    secret: zod_1.z.string().optional(),
    endpoints: zod_1.z.array(zod_1.z.string().url()).default([]),
});
exports.APIConfigSchema = zod_1.z.object({
    enabled: zod_1.z.boolean().default(true),
    port: zod_1.z.number().int().min(1).max(65535).default(3000),
    host: zod_1.z.string().default('0.0.0.0'),
    cors: zod_1.z.boolean().default(true),
    rateLimit: zod_1.z.object({
        max: zod_1.z.number().int().default(100),
        timeWindow: zod_1.z.number().int().default(60000),
    }).default({}),
    auth: zod_1.z.object({
        enabled: zod_1.z.boolean().default(false),
        type: zod_1.z.enum(['bearer', 'basic', 'apikey']).default('bearer'),
        secret: zod_1.z.string().optional(),
    }).default({}),
});
exports.StorageConfigSchema = zod_1.z.object({
    type: zod_1.z.enum(['sqlite', 'postgres', 'memory']).default('sqlite'),
    path: zod_1.z.string().default('./data/ruvbot.db'),
    connectionString: zod_1.z.string().optional(),
    poolSize: zod_1.z.number().int().min(1).max(100).default(10),
});
exports.LoggingConfigSchema = zod_1.z.object({
    level: zod_1.z.enum(['trace', 'debug', 'info', 'warn', 'error', 'fatal']).default('info'),
    pretty: zod_1.z.boolean().default(true),
    file: zod_1.z.string().optional(),
});
exports.BotConfigSchema = zod_1.z.object({
    name: zod_1.z.string().min(1).max(64).default('RuvBot'),
    version: zod_1.z.string().default('0.1.0'),
    environment: zod_1.z.enum(['development', 'staging', 'production']).default('development'),
    // Core settings
    memory: exports.MemoryConfigSchema.default({}),
    llm: exports.LLMConfigSchema.default({}),
    storage: exports.StorageConfigSchema.default({}),
    logging: exports.LoggingConfigSchema.default({}),
    api: exports.APIConfigSchema.default({}),
    // Integrations
    slack: exports.SlackConfigSchema.default({}),
    discord: exports.DiscordConfigSchema.default({}),
    webhook: exports.WebhookConfigSchema.default({}),
    // Skills
    skills: zod_1.z.object({
        enabled: zod_1.z.array(zod_1.z.string()).default(['search', 'summarize', 'code', 'memory']),
        custom: zod_1.z.array(zod_1.z.string()).default([]),
        directory: zod_1.z.string().default('./skills'),
    }).default({}),
    // Session settings
    session: zod_1.z.object({
        defaultTTL: zod_1.z.number().int().min(60000).default(3600000), // 1 hour
        maxPerUser: zod_1.z.number().int().min(1).max(100).default(10),
        maxMessages: zod_1.z.number().int().min(10).max(10000).default(1000),
    }).default({}),
    // Worker settings
    workers: zod_1.z.object({
        poolSize: zod_1.z.number().int().min(1).max(50).default(4),
        taskTimeout: zod_1.z.number().int().min(1000).default(30000),
        retryAttempts: zod_1.z.number().int().min(0).max(10).default(3),
    }).default({}),
});
// ============================================================================
// Configuration Manager
// ============================================================================
class ConfigManager {
    constructor(initialConfig) {
        this.config = exports.BotConfigSchema.parse(initialConfig ?? {});
    }
    /**
     * Get the full configuration
     */
    getConfig() {
        return Object.freeze({ ...this.config });
    }
    /**
     * Get a specific configuration section
     */
    get(key) {
        return this.config[key];
    }
    /**
     * Update configuration
     */
    update(updates) {
        this.config = exports.BotConfigSchema.parse({
            ...this.config,
            ...updates,
        });
    }
    /**
     * Validate configuration
     */
    validate() {
        try {
            exports.BotConfigSchema.parse(this.config);
            return { valid: true, errors: [] };
        }
        catch (error) {
            if (error instanceof zod_1.z.ZodError) {
                return {
                    valid: false,
                    errors: error.errors.map((e) => `${e.path.join('.')}: ${e.message}`),
                };
            }
            throw error;
        }
    }
    /**
     * Load configuration from environment variables
     */
    static fromEnv() {
        // Build partial config - Zod will apply defaults
        const llmConfig = {};
        const slackConfig = {};
        const discordConfig = {};
        const apiConfig = {};
        const storageConfig = {};
        const loggingConfig = {};
        // LLM configuration
        if (process.env.ANTHROPIC_API_KEY) {
            llmConfig.provider = 'anthropic';
            llmConfig.apiKey = process.env.ANTHROPIC_API_KEY;
        }
        else if (process.env.OPENAI_API_KEY) {
            llmConfig.provider = 'openai';
            llmConfig.apiKey = process.env.OPENAI_API_KEY;
        }
        // Slack configuration
        if (process.env.SLACK_BOT_TOKEN) {
            slackConfig.enabled = true;
            slackConfig.botToken = process.env.SLACK_BOT_TOKEN;
            slackConfig.signingSecret = process.env.SLACK_SIGNING_SECRET;
            slackConfig.appToken = process.env.SLACK_APP_TOKEN;
            slackConfig.socketMode = true;
        }
        // Discord configuration
        if (process.env.DISCORD_TOKEN) {
            discordConfig.enabled = true;
            discordConfig.token = process.env.DISCORD_TOKEN;
            discordConfig.clientId = process.env.DISCORD_CLIENT_ID;
            discordConfig.guildId = process.env.DISCORD_GUILD_ID;
        }
        // API configuration
        if (process.env.RUVBOT_PORT) {
            apiConfig.port = parseInt(process.env.RUVBOT_PORT, 10);
        }
        // Storage configuration
        if (process.env.DATABASE_URL) {
            storageConfig.type = 'postgres';
            storageConfig.connectionString = process.env.DATABASE_URL;
        }
        // Logging
        if (process.env.RUVBOT_LOG_LEVEL) {
            loggingConfig.level = process.env.RUVBOT_LOG_LEVEL;
        }
        const config = {};
        if (Object.keys(llmConfig).length > 0)
            config.llm = llmConfig;
        if (Object.keys(slackConfig).length > 0)
            config.slack = slackConfig;
        if (Object.keys(discordConfig).length > 0)
            config.discord = discordConfig;
        if (Object.keys(apiConfig).length > 0)
            config.api = apiConfig;
        if (Object.keys(storageConfig).length > 0)
            config.storage = storageConfig;
        if (Object.keys(loggingConfig).length > 0)
            config.logging = loggingConfig;
        return new ConfigManager(config);
    }
    /**
     * Export configuration as JSON
     */
    toJSON() {
        return JSON.stringify(this.config, null, 2);
    }
}
exports.ConfigManager = ConfigManager;
//# sourceMappingURL=BotConfig.js.map