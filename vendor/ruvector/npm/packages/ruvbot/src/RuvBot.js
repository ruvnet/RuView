"use strict";
/**
 * RuvBot - Self-learning AI Assistant with RuVector Backend
 *
 * Main entry point for the RuvBot framework.
 * Combines Clawdbot-style personal AI with RuVector's WASM vector operations.
 */
var __importDefault = (this && this.__importDefault) || function (mod) {
    return (mod && mod.__esModule) ? mod : { "default": mod };
};
Object.defineProperty(exports, "__esModule", { value: true });
exports.RuvBot = void 0;
exports.createRuvBot = createRuvBot;
exports.createRuvBotFromEnv = createRuvBotFromEnv;
const eventemitter3_1 = require("eventemitter3");
const node_http_1 = require("node:http");
const pino_1 = __importDefault(require("pino"));
const uuid_1 = require("uuid");
const BotConfig_js_1 = require("./core/BotConfig.js");
const BotState_js_1 = require("./core/BotState.js");
const errors_js_1 = require("./core/errors.js");
const index_js_1 = require("./integration/providers/index.js");
// ============================================================================
// RuvBot Main Class
// ============================================================================
class RuvBot extends eventemitter3_1.EventEmitter {
    constructor(options = {}) {
        super();
        this.agents = new Map();
        this.sessions = new Map();
        this.isRunning = false;
        this.llmProvider = null;
        this.httpServer = null;
        this.id = (0, uuid_1.v4)();
        // Initialize configuration
        if (options.config) {
            this.configManager = new BotConfig_js_1.ConfigManager(options.config);
        }
        else {
            this.configManager = BotConfig_js_1.ConfigManager.fromEnv();
        }
        // Validate configuration
        const validation = this.configManager.validate();
        if (!validation.valid) {
            throw new errors_js_1.ConfigurationError(`Invalid configuration: ${validation.errors.join(', ')}`);
        }
        // Initialize logger
        const config = this.configManager.getConfig();
        this.logger = (0, pino_1.default)({
            level: config.logging.level,
            transport: config.logging.pretty
                ? { target: 'pino-pretty', options: { colorize: true } }
                : undefined,
        });
        // Initialize state manager
        this.stateManager = new BotState_js_1.BotStateManager();
        this.logger.info({ botId: this.id }, 'RuvBot instance created');
        // Auto-start if requested
        if (options.autoStart) {
            this.start().catch((error) => {
                this.logger.error({ error }, 'Auto-start failed');
                this.emit('error', error);
            });
        }
    }
    // ==========================================================================
    // Lifecycle Methods
    // ==========================================================================
    /**
     * Start the bot and all configured services
     */
    async start() {
        if (this.isRunning) {
            this.logger.warn('RuvBot is already running');
            return;
        }
        this.logger.info('Starting RuvBot...');
        this.stateManager.setStatus('starting');
        try {
            const config = this.configManager.getConfig();
            // Initialize core services
            await this.initializeServices();
            // Start integrations
            await this.startIntegrations(config);
            // Start API server if enabled
            if (config.api.enabled) {
                await this.startApiServer(config);
            }
            // Mark as running
            this.isRunning = true;
            this.startTime = new Date();
            this.stateManager.setStatus('running');
            this.logger.info({ botId: this.id, name: config.name }, 'RuvBot started successfully');
            this.emit('ready');
        }
        catch (error) {
            this.stateManager.setStatus('error');
            this.logger.error({ error }, 'Failed to start RuvBot');
            throw new errors_js_1.InitializationError(`Failed to start RuvBot: ${error instanceof Error ? error.message : 'Unknown error'}`);
        }
    }
    /**
     * Stop the bot and cleanup resources
     */
    async stop() {
        if (!this.isRunning) {
            this.logger.warn('RuvBot is not running');
            return;
        }
        this.logger.info('Stopping RuvBot...');
        this.stateManager.setStatus('stopping');
        try {
            // Stop all agents
            for (const [agentId] of this.agents) {
                await this.stopAgent(agentId);
            }
            // End all sessions
            for (const [sessionId] of this.sessions) {
                await this.endSession(sessionId);
            }
            // Stop integrations
            await this.stopIntegrations();
            // Stop API server
            await this.stopApiServer();
            this.isRunning = false;
            this.stateManager.setStatus('stopped');
            this.logger.info('RuvBot stopped successfully');
            this.emit('shutdown');
        }
        catch (error) {
            this.stateManager.setStatus('error');
            this.logger.error({ error }, 'Error during shutdown');
            throw error;
        }
    }
    // ==========================================================================
    // Agent Management
    // ==========================================================================
    /**
     * Spawn a new agent with the given configuration
     */
    async spawnAgent(config) {
        const agentId = config.id || (0, uuid_1.v4)();
        if (this.agents.has(agentId)) {
            throw new errors_js_1.RuvBotError(`Agent with ID ${agentId} already exists`, 'AGENT_EXISTS');
        }
        const agent = {
            id: agentId,
            name: config.name,
            config,
            status: 'idle',
            createdAt: new Date(),
            lastActiveAt: new Date(),
        };
        this.agents.set(agentId, agent);
        this.logger.info({ agentId, name: config.name }, 'Agent spawned');
        this.emit('agent:spawn', agent);
        return agent;
    }
    /**
     * Stop an agent by ID
     */
    async stopAgent(agentId) {
        const agent = this.agents.get(agentId);
        if (!agent) {
            throw new errors_js_1.RuvBotError(`Agent with ID ${agentId} not found`, 'AGENT_NOT_FOUND');
        }
        // End all sessions for this agent
        for (const [sessionId, session] of this.sessions) {
            if (session.agentId === agentId) {
                await this.endSession(sessionId);
            }
        }
        this.agents.delete(agentId);
        this.logger.info({ agentId }, 'Agent stopped');
        this.emit('agent:stop', agentId);
    }
    /**
     * Get an agent by ID
     */
    getAgent(agentId) {
        return this.agents.get(agentId);
    }
    /**
     * List all active agents
     */
    listAgents() {
        return Array.from(this.agents.values());
    }
    // ==========================================================================
    // Session Management
    // ==========================================================================
    /**
     * Create a new session for an agent
     */
    async createSession(agentId, options = {}) {
        const agent = this.agents.get(agentId);
        if (!agent) {
            throw new errors_js_1.RuvBotError(`Agent with ID ${agentId} not found`, 'AGENT_NOT_FOUND');
        }
        const sessionId = (0, uuid_1.v4)();
        const config = this.configManager.getConfig();
        const session = {
            id: sessionId,
            agentId,
            userId: options.userId,
            channelId: options.channelId,
            platform: options.platform || 'api',
            messages: [],
            context: {
                topics: [],
                entities: [],
            },
            metadata: options.metadata || {},
            createdAt: new Date(),
            updatedAt: new Date(),
            expiresAt: new Date(Date.now() + config.session.defaultTTL),
        };
        this.sessions.set(sessionId, session);
        this.logger.info({ sessionId, agentId }, 'Session created');
        this.emit('session:create', session);
        return session;
    }
    /**
     * End a session by ID
     */
    async endSession(sessionId) {
        const session = this.sessions.get(sessionId);
        if (!session) {
            throw new errors_js_1.RuvBotError(`Session with ID ${sessionId} not found`, 'SESSION_NOT_FOUND');
        }
        this.sessions.delete(sessionId);
        this.logger.info({ sessionId }, 'Session ended');
        this.emit('session:end', sessionId);
    }
    /**
     * Get a session by ID
     */
    getSession(sessionId) {
        return this.sessions.get(sessionId);
    }
    /**
     * List all active sessions
     */
    listSessions() {
        return Array.from(this.sessions.values());
    }
    // ==========================================================================
    // Message Handling
    // ==========================================================================
    /**
     * Send a message to an agent in a session
     */
    async chat(sessionId, content, options = {}) {
        const session = this.sessions.get(sessionId);
        if (!session) {
            throw new errors_js_1.RuvBotError(`Session with ID ${sessionId} not found`, 'SESSION_NOT_FOUND');
        }
        const agent = this.agents.get(session.agentId);
        if (!agent) {
            throw new errors_js_1.RuvBotError(`Agent with ID ${session.agentId} not found`, 'AGENT_NOT_FOUND');
        }
        // Create user message
        const userMessage = {
            id: (0, uuid_1.v4)(),
            sessionId,
            role: 'user',
            content,
            attachments: options.attachments,
            metadata: options.metadata,
            createdAt: new Date(),
        };
        // Add to session
        session.messages.push(userMessage);
        session.updatedAt = new Date();
        // Update agent status
        agent.status = 'processing';
        agent.lastActiveAt = new Date();
        this.logger.debug({ sessionId, messageId: userMessage.id }, 'User message received');
        this.emit('message', userMessage, session);
        try {
            // Generate response (placeholder for LLM integration)
            const responseContent = await this.generateResponse(session, agent, content);
            // Create assistant message
            const assistantMessage = {
                id: (0, uuid_1.v4)(),
                sessionId,
                role: 'assistant',
                content: responseContent,
                createdAt: new Date(),
            };
            session.messages.push(assistantMessage);
            session.updatedAt = new Date();
            agent.status = 'idle';
            this.logger.debug({ sessionId, messageId: assistantMessage.id }, 'Assistant response generated');
            this.emit('message', assistantMessage, session);
            return assistantMessage;
        }
        catch (error) {
            agent.status = 'error';
            throw error;
        }
    }
    // ==========================================================================
    // Status & Info
    // ==========================================================================
    /**
     * Get the current bot status
     */
    getStatus() {
        const config = this.configManager.getConfig();
        return {
            id: this.id,
            name: config.name,
            state: this.stateManager.getStatus(),
            isRunning: this.isRunning,
            uptime: this.startTime
                ? Date.now() - this.startTime.getTime()
                : undefined,
            agents: this.agents.size,
            sessions: this.sessions.size,
        };
    }
    /**
     * Get the current configuration
     */
    getConfig() {
        return this.configManager.getConfig();
    }
    // ==========================================================================
    // Private Methods
    // ==========================================================================
    async initializeServices() {
        this.logger.debug('Initializing core services...');
        const config = this.configManager.getConfig();
        // Initialize LLM provider based on configuration
        const { provider, apiKey, model } = config.llm;
        // Check for available API keys in priority order
        const openrouterKey = process.env.OPENROUTER_API_KEY;
        const anthropicKey = process.env.ANTHROPIC_API_KEY || apiKey;
        const googleAIKey = process.env.GOOGLE_AI_API_KEY || process.env.GEMINI_API_KEY;
        if (openrouterKey) {
            // Use OpenRouter for Gemini 2.5 and other models
            this.llmProvider = (0, index_js_1.createOpenRouterProvider)({
                apiKey: openrouterKey,
                model: model || 'google/gemini-2.5-pro-preview-05-06',
                siteName: 'RuvBot',
            });
            this.logger.info({ provider: 'openrouter', model: model || 'google/gemini-2.5-pro-preview-05-06' }, 'LLM provider initialized');
        }
        else if (googleAIKey) {
            // Use Google AI directly (Gemini 2.5)
            this.llmProvider = (0, index_js_1.createGoogleAIProvider)({
                apiKey: googleAIKey,
                model: model || 'gemini-2.5-flash',
            });
            this.logger.info({ provider: 'google-ai', model: model || 'gemini-2.5-flash' }, 'LLM provider initialized');
        }
        else if (provider === 'anthropic' && anthropicKey) {
            this.llmProvider = (0, index_js_1.createAnthropicProvider)({
                apiKey: anthropicKey,
                model: model || 'claude-3-5-sonnet-20241022',
            });
            this.logger.info({ provider: 'anthropic', model }, 'LLM provider initialized');
        }
        else if (anthropicKey) {
            // Fallback to Anthropic if only that key is available
            this.llmProvider = (0, index_js_1.createAnthropicProvider)({
                apiKey: anthropicKey,
                model: model || 'claude-3-5-sonnet-20241022',
            });
            this.logger.info({ provider: 'anthropic', model }, 'LLM provider initialized');
        }
        else {
            this.logger.warn({}, 'No LLM API key found. Set GOOGLE_AI_API_KEY, ANTHROPIC_API_KEY, or OPENROUTER_API_KEY');
        }
        // TODO: Initialize memory manager, skill registry, etc.
    }
    async startIntegrations(config) {
        this.logger.debug('Starting integrations...');
        if (config.slack.enabled) {
            this.logger.info('Slack integration enabled');
            // TODO: Initialize Slack adapter
        }
        if (config.discord.enabled) {
            this.logger.info('Discord integration enabled');
            // TODO: Initialize Discord adapter
        }
        if (config.webhook.enabled) {
            this.logger.info('Webhook integration enabled');
            // TODO: Initialize webhook handler
        }
    }
    async stopIntegrations() {
        this.logger.debug('Stopping integrations...');
        // TODO: Stop all integration adapters
    }
    async startApiServer(config) {
        const port = config.api.port || 3000;
        const host = config.api.host || '0.0.0.0';
        this.httpServer = (0, node_http_1.createServer)((req, res) => {
            this.handleApiRequest(req, res).catch((error) => {
                this.logger.error({ err: error }, 'Unhandled API request error');
                if (!res.headersSent) {
                    res.writeHead(500, { 'Content-Type': 'application/json' });
                    res.end(JSON.stringify({ error: 'Internal server error' }));
                }
            });
        });
        return new Promise((resolve, reject) => {
            this.httpServer.on('error', (err) => {
                this.logger.error({ err, port, host }, 'API server failed to start');
                reject(err);
            });
            this.httpServer.listen(port, host, () => {
                this.logger.info({ port, host }, 'API server listening');
                resolve();
            });
        });
    }
    async stopApiServer() {
        if (!this.httpServer)
            return;
        return new Promise((resolve) => {
            this.httpServer.close(() => {
                this.logger.debug('API server stopped');
                this.httpServer = null;
                resolve();
            });
        });
    }
    async handleApiRequest(req, res) {
        const url = new URL(req.url || '/', `http://${req.headers.host || 'localhost'}`);
        const path = url.pathname;
        const method = req.method || 'GET';
        // CORS headers
        res.setHeader('Access-Control-Allow-Origin', '*');
        res.setHeader('Access-Control-Allow-Methods', 'GET, POST, OPTIONS');
        res.setHeader('Access-Control-Allow-Headers', 'Content-Type, Authorization');
        if (method === 'OPTIONS') {
            res.writeHead(204);
            res.end();
            return;
        }
        const json = (status, data) => {
            res.writeHead(status, { 'Content-Type': 'application/json' });
            res.end(JSON.stringify(data));
        };
        // Health check
        if (path === '/health' || path === '/healthz') {
            json(200, {
                status: 'healthy',
                uptime: this.startTime ? Math.floor((Date.now() - this.startTime.getTime()) / 1000) : 0,
                timestamp: new Date().toISOString(),
            });
            return;
        }
        // Readiness check
        if (path === '/ready' || path === '/readyz') {
            if (this.isRunning) {
                json(200, { status: 'ready' });
            }
            else {
                json(503, { status: 'not ready' });
            }
            return;
        }
        // Status
        if (path === '/api/status') {
            json(200, this.getStatus());
            return;
        }
        // Chat endpoint
        if (path === '/api/chat' && method === 'POST') {
            const body = await this.parseRequestBody(req);
            const message = body?.message;
            const agentId = body?.agentId || 'default-agent';
            if (!message) {
                json(400, { error: 'Missing "message" field' });
                return;
            }
            // Create or reuse a session
            let sessionId = body?.sessionId;
            if (!sessionId || !this.sessions.has(sessionId)) {
                const session = await this.createSession(agentId);
                sessionId = session.id;
            }
            const response = await this.chat(sessionId, message);
            json(200, { sessionId, agentId, response });
            return;
        }
        // List agents
        if (path === '/api/agents' && method === 'GET') {
            json(200, { agents: this.listAgents() });
            return;
        }
        // List sessions
        if (path === '/api/sessions' && method === 'GET') {
            json(200, { sessions: this.listSessions() });
            return;
        }
        // Root — simple landing page
        if (path === '/') {
            res.writeHead(200, { 'Content-Type': 'text/html' });
            res.end(`<!DOCTYPE html><html><head><title>RuvBot</title>
        <style>body{font-family:system-ui;background:#0a0a0f;color:#f0f0f5;display:flex;align-items:center;justify-content:center;min-height:100vh;margin:0}
        .c{text-align:center}h1{font-size:3rem}p{color:#a0a0b0}a{color:#6366f1;text-decoration:none;padding:12px 24px;border:1px solid #6366f1;border-radius:8px;display:inline-block}a:hover{background:#6366f1;color:#fff}</style>
        </head><body><div class="c"><h1>RuvBot</h1><p>Enterprise-grade AI Assistant</p><a href="/api/status">API Status</a></div></body></html>`);
            return;
        }
        // 404
        json(404, { error: 'Not found' });
    }
    parseRequestBody(req) {
        return new Promise((resolve, reject) => {
            const chunks = [];
            req.on('data', (chunk) => chunks.push(chunk));
            req.on('end', () => {
                if (chunks.length === 0) {
                    resolve(null);
                    return;
                }
                try {
                    resolve(JSON.parse(Buffer.concat(chunks).toString('utf-8')));
                }
                catch {
                    reject(new Error('Invalid JSON'));
                }
            });
            req.on('error', reject);
        });
    }
    async generateResponse(session, agent, userMessage) {
        // If no LLM provider, return helpful error message
        if (!this.llmProvider) {
            this.logger.warn('No LLM provider configured');
            return `**LLM Not Configured**

To enable AI responses, please set one of these environment variables:

- \`GOOGLE_AI_API_KEY\` - Get from [Google AI Studio](https://aistudio.google.com/app/apikey)
- \`ANTHROPIC_API_KEY\` - Get from [Anthropic Console](https://console.anthropic.com/)
- \`OPENROUTER_API_KEY\` - Get from [OpenRouter](https://openrouter.ai/)

Then redeploy the service with the API key set.

*Your message was: "${userMessage}"*`;
        }
        // Build message history for context
        const messages = [];
        // Add system prompt from agent config
        if (agent.config.systemPrompt) {
            messages.push({
                role: 'system',
                content: agent.config.systemPrompt,
            });
        }
        // Add recent message history (last 20 messages for context)
        const recentMessages = session.messages.slice(-20);
        for (const msg of recentMessages) {
            messages.push({
                role: msg.role === 'user' ? 'user' : 'assistant',
                content: msg.content,
            });
        }
        // Add current user message
        messages.push({
            role: 'user',
            content: userMessage,
        });
        try {
            // Call LLM provider
            const completion = await this.llmProvider.complete(messages, {
                temperature: agent.config.temperature ?? 0.7,
                maxTokens: agent.config.maxTokens ?? 4096,
            });
            this.logger.debug({
                inputTokens: completion.usage.inputTokens,
                outputTokens: completion.usage.outputTokens,
                finishReason: completion.finishReason,
            }, 'LLM response received');
            return completion.content;
        }
        catch (error) {
            this.logger.error({ error }, 'LLM completion failed');
            throw new errors_js_1.RuvBotError(`Failed to generate response: ${error instanceof Error ? error.message : 'Unknown error'}`, 'LLM_ERROR');
        }
    }
}
exports.RuvBot = RuvBot;
// ============================================================================
// Factory Functions
// ============================================================================
/**
 * Create a new RuvBot instance
 */
function createRuvBot(options) {
    return new RuvBot(options);
}
/**
 * Create a RuvBot instance from environment variables
 */
function createRuvBotFromEnv() {
    return new RuvBot();
}
exports.default = RuvBot;
//# sourceMappingURL=RuvBot.js.map