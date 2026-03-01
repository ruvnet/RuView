"use strict";
/**
 * RuvBot HTTP Server - Cloud Run Entry Point
 *
 * Provides REST API endpoints for RuvBot including:
 * - Health checks (required for Cloud Run)
 * - Chat API
 * - Session management
 * - Agent management
 */
var __importDefault = (this && this.__importDefault) || function (mod) {
    return (mod && mod.__esModule) ? mod : { "default": mod };
};
Object.defineProperty(exports, "__esModule", { value: true });
const node_http_1 = require("node:http");
const node_url_1 = require("node:url");
const node_crypto_1 = require("node:crypto");
const node_fs_1 = require("node:fs");
const node_path_1 = require("node:path");
const pino_1 = __importDefault(require("pino"));
const RuvBot_js_1 = require("./RuvBot.js");
const AIDefenceGuard_js_1 = require("./security/AIDefenceGuard.js");
const ChatEnhancer_js_1 = require("./core/ChatEnhancer.js");
// ============================================================================
// Configuration
// ============================================================================
const PORT = parseInt(process.env.PORT || '8080', 10);
const HOST = process.env.HOST || '0.0.0.0';
const NODE_ENV = process.env.NODE_ENV || 'development';
const logger = (0, pino_1.default)({
    level: process.env.LOG_LEVEL || 'info',
    transport: NODE_ENV !== 'production'
        ? { target: 'pino-pretty', options: { colorize: true } }
        : undefined,
});
// ============================================================================
// Server State
// ============================================================================
let bot = null;
let aiDefence = null;
let chatEnhancer = null;
const startTime = Date.now();
// ============================================================================
// Utility Functions
// ============================================================================
async function parseBody(req) {
    return new Promise((resolve, reject) => {
        const chunks = [];
        req.on('data', (chunk) => chunks.push(chunk));
        req.on('end', () => {
            if (chunks.length === 0) {
                resolve(null);
                return;
            }
            const rawBody = Buffer.concat(chunks).toString('utf-8');
            try {
                const body = JSON.parse(rawBody);
                resolve(body);
            }
            catch (parseError) {
                logger.error({
                    rawBody: rawBody.substring(0, 500),
                    contentType: req.headers['content-type'],
                    contentLength: req.headers['content-length'],
                    err: parseError
                }, 'JSON parse error');
                reject(new Error('Invalid JSON'));
            }
        });
        req.on('error', (err) => {
            logger.error({ err }, 'Request body read error');
            reject(err);
        });
    });
}
function sendJSON(res, statusCode, data) {
    res.writeHead(statusCode, {
        'Content-Type': 'application/json',
        'X-Content-Type-Options': 'nosniff',
    });
    res.end(JSON.stringify(data));
}
function sendError(res, statusCode, message, code) {
    sendJSON(res, statusCode, { error: message, code: code || 'ERROR' });
}
// ============================================================================
// Static File Serving
// ============================================================================
function serveStaticFile(res, filePath, contentType) {
    try {
        const content = (0, node_fs_1.readFileSync)(filePath, 'utf-8');
        res.writeHead(200, {
            'Content-Type': contentType,
            'Cache-Control': 'public, max-age=3600',
        });
        res.end(content);
        return true;
    }
    catch {
        return false;
    }
}
function getChatUIPath() {
    // Try multiple locations for the chat UI
    // Works in both development (src/) and production (dist/)
    const cwd = process.cwd();
    const possiblePaths = [
        // Docker/Cloud Run paths (WORKDIR /app)
        (0, node_path_1.join)(cwd, 'dist', 'api', 'public', 'index.html'),
        // Development paths
        (0, node_path_1.join)(cwd, 'src', 'api', 'public', 'index.html'),
        // Production paths (ESM)
        (0, node_path_1.join)(cwd, 'dist', 'esm', 'api', 'public', 'index.html'),
        // When running from node_modules
        (0, node_path_1.join)(cwd, 'node_modules', 'ruvbot', 'dist', 'api', 'public', 'index.html'),
        (0, node_path_1.join)(cwd, 'node_modules', 'ruvbot', 'src', 'api', 'public', 'index.html'),
        // Absolute paths (for Docker)
        '/app/dist/api/public/index.html',
        '/app/src/api/public/index.html',
    ];
    for (const p of possiblePaths) {
        if ((0, node_fs_1.existsSync)(p)) {
            logger.info({ path: p }, 'Found chat UI');
            return p;
        }
    }
    logger.warn({ cwd, paths: possiblePaths }, 'Chat UI not found, using fallback');
    return possiblePaths[0]; // Default to first path
}
// ============================================================================
// Route Handlers
// ============================================================================
async function handleRoot(ctx) {
    const { res } = ctx;
    const chatUIPath = getChatUIPath();
    if ((0, node_fs_1.existsSync)(chatUIPath)) {
        serveStaticFile(res, chatUIPath, 'text/html; charset=utf-8');
    }
    else {
        // Fallback: serve a simple redirect or message
        res.writeHead(200, { 'Content-Type': 'text/html' });
        res.end(`
      <!DOCTYPE html>
      <html>
      <head>
        <title>RuvBot</title>
        <style>
          body { font-family: system-ui; background: #0a0a0f; color: #f0f0f5; display: flex; align-items: center; justify-content: center; min-height: 100vh; margin: 0; }
          .container { text-align: center; }
          h1 { font-size: 3rem; margin-bottom: 1rem; }
          p { color: #a0a0b0; margin-bottom: 2rem; }
          a { color: #6366f1; text-decoration: none; padding: 12px 24px; border: 1px solid #6366f1; border-radius: 8px; display: inline-block; }
          a:hover { background: #6366f1; color: white; }
        </style>
      </head>
      <body>
        <div class="container">
          <h1>🤖 RuvBot</h1>
          <p>Enterprise-grade AI Assistant</p>
          <a href="/api/status">View API Status</a>
        </div>
      </body>
      </html>
    `);
    }
}
async function handleHealth(ctx) {
    const { res } = ctx;
    sendJSON(res, 200, {
        status: 'healthy',
        version: '0.2.0',
        uptime: Math.floor((Date.now() - startTime) / 1000),
        timestamp: new Date().toISOString(),
    });
}
async function handleReady(ctx) {
    const { res } = ctx;
    if (bot?.getStatus().isRunning) {
        sendJSON(res, 200, { status: 'ready' });
    }
    else {
        sendError(res, 503, 'Service not ready', 'NOT_READY');
    }
}
async function handleStatus(ctx) {
    const { res } = ctx;
    if (!bot) {
        sendError(res, 503, 'Bot not initialized', 'NOT_INITIALIZED');
        return;
    }
    const status = bot.getStatus();
    const config = bot.getConfig();
    // Check LLM configuration
    const hasAnthropicKey = !!(process.env.ANTHROPIC_API_KEY || config.llm?.apiKey);
    const hasOpenRouterKey = !!process.env.OPENROUTER_API_KEY;
    const hasGoogleAIKey = !!(process.env.GOOGLE_AI_API_KEY || process.env.GEMINI_API_KEY);
    const hasAnyKey = hasAnthropicKey || hasOpenRouterKey || hasGoogleAIKey;
    // Determine active provider
    let activeProvider = 'none';
    if (hasOpenRouterKey)
        activeProvider = 'openrouter';
    else if (hasGoogleAIKey)
        activeProvider = 'google-ai';
    else if (hasAnthropicKey)
        activeProvider = 'anthropic';
    sendJSON(res, 200, {
        ...status,
        llm: {
            configured: hasAnyKey,
            provider: activeProvider,
            model: config.llm?.model || 'not set',
            hasApiKey: hasAnyKey,
        },
        environment: {
            nodeEnv: NODE_ENV,
            hasAnthropicKey,
            hasOpenRouterKey,
            hasGoogleAIKey,
        },
    });
}
async function handleSkills(ctx) {
    const { res } = ctx;
    if (!chatEnhancer) {
        sendJSON(res, 200, { skills: [], message: 'ChatEnhancer not initialized' });
        return;
    }
    const skills = chatEnhancer.getAvailableSkills();
    const memoryStats = chatEnhancer.getMemoryStats();
    sendJSON(res, 200, {
        skills,
        categories: {
            search: skills.filter(s => s.id.includes('search')),
            memory: skills.filter(s => s.id.includes('memory')),
            code: skills.filter(s => s.id.includes('code')),
            summarize: skills.filter(s => s.id.includes('summar')),
        },
        memoryStats,
        usage: 'Include skill trigger words in your message to automatically invoke skills.',
        examples: [
            'search for TypeScript async patterns',
            'remember that my project uses React 18',
            'explain this code: function add(a, b) { return a + b; }',
            'summarize our conversation',
        ],
    });
}
async function handleModels(ctx) {
    const { res } = ctx;
    sendJSON(res, 200, {
        models: [
            // Gemini 2.x (recommended)
            { id: 'google/gemini-2.5-pro-preview-05-06', name: 'Gemini 2.5 Pro Preview', provider: 'openrouter' },
            { id: 'google/gemini-2.0-flash-001', name: 'Gemini 2.0 Flash', provider: 'openrouter' },
            { id: 'google/gemini-2.0-flash-thinking-exp:free', name: 'Gemini 2.0 Flash Thinking (Free)', provider: 'openrouter' },
            // Anthropic Claude
            { id: 'anthropic/claude-3.5-sonnet', name: 'Claude 3.5 Sonnet', provider: 'openrouter' },
            { id: 'anthropic/claude-3-opus', name: 'Claude 3 Opus', provider: 'openrouter' },
            { id: 'claude-3-5-sonnet-20241022', name: 'Claude 3.5 Sonnet (Direct)', provider: 'anthropic' },
            // OpenAI
            { id: 'openai/gpt-4o', name: 'GPT-4o', provider: 'openrouter' },
            { id: 'openai/o1-preview', name: 'O1 Preview (Reasoning)', provider: 'openrouter' },
            // Qwen
            { id: 'qwen/qwq-32b', name: 'Qwen QwQ 32B (Reasoning)', provider: 'openrouter' },
            { id: 'qwen/qwq-32b:free', name: 'Qwen QwQ 32B (Free)', provider: 'openrouter' },
            // DeepSeek
            { id: 'deepseek/deepseek-r1', name: 'DeepSeek R1 (Reasoning)', provider: 'openrouter' },
            // Meta
            { id: 'meta-llama/llama-3.1-405b-instruct', name: 'Llama 3.1 405B', provider: 'openrouter' },
        ],
        default: 'google/gemini-2.5-pro-preview-05-06',
    });
}
async function handleCreateAgent(ctx) {
    const { res, body } = ctx;
    if (!bot) {
        sendError(res, 503, 'Bot not initialized', 'NOT_INITIALIZED');
        return;
    }
    if (!body || typeof body.name !== 'string') {
        sendError(res, 400, 'Agent name is required', 'INVALID_REQUEST');
        return;
    }
    const config = {
        id: body.id || (0, node_crypto_1.randomUUID)(),
        name: body.name,
        model: body.model || 'claude-3-haiku-20240307',
        systemPrompt: body.systemPrompt,
        temperature: body.temperature,
        maxTokens: body.maxTokens,
    };
    const agent = await bot.spawnAgent(config);
    sendJSON(res, 201, agent);
}
async function handleListAgents(ctx) {
    const { res } = ctx;
    if (!bot) {
        sendError(res, 503, 'Bot not initialized', 'NOT_INITIALIZED');
        return;
    }
    sendJSON(res, 200, { agents: bot.listAgents() });
}
async function handleCreateSession(ctx) {
    const { res, body } = ctx;
    if (!bot) {
        sendError(res, 503, 'Bot not initialized', 'NOT_INITIALIZED');
        return;
    }
    if (!body || typeof body.agentId !== 'string') {
        sendError(res, 400, 'Agent ID is required', 'INVALID_REQUEST');
        return;
    }
    try {
        const session = await bot.createSession(body.agentId, {
            userId: body.userId,
            channelId: body.channelId,
            platform: body.platform,
            metadata: body.metadata,
        });
        sendJSON(res, 201, session);
    }
    catch (error) {
        const message = error instanceof Error ? error.message : 'Unknown error';
        sendError(res, 400, message, 'SESSION_ERROR');
    }
}
async function handleListSessions(ctx) {
    const { res } = ctx;
    if (!bot) {
        sendError(res, 503, 'Bot not initialized', 'NOT_INITIALIZED');
        return;
    }
    sendJSON(res, 200, { sessions: bot.listSessions() });
}
async function handleChat(ctx) {
    const { res, body, url } = ctx;
    if (!bot) {
        sendError(res, 503, 'Bot not initialized', 'NOT_INITIALIZED');
        return;
    }
    const sessionId = url.pathname.split('/')[3]; // /api/sessions/:id/chat
    if (!sessionId) {
        sendError(res, 400, 'Session ID is required', 'INVALID_REQUEST');
        return;
    }
    if (!body || typeof body.message !== 'string') {
        sendError(res, 400, 'Message is required', 'INVALID_REQUEST');
        return;
    }
    // Validate input with AIDefence if enabled
    let messageContent = body.message;
    let inputBlocked = false;
    if (aiDefence) {
        const analysisResult = await aiDefence.analyze(messageContent);
        if (!analysisResult.safe) {
            logger.warn({
                threats: analysisResult.threats,
                threatLevel: analysisResult.threatLevel,
            }, 'Threats detected in message');
            // Block threats that exceed the configured threshold (safe = false)
            // This includes critical, high, and medium threats based on blockThreshold
            sendError(res, 400, 'Message blocked due to security concerns', 'SECURITY_BLOCKED');
            inputBlocked = true;
            return;
        }
    }
    if (inputBlocked)
        return;
    try {
        logger.debug({ sessionId, messageLength: messageContent.length }, 'Processing chat request');
        // Step 1: Process with ChatEnhancer for skills and memory
        let enhancedContext = '';
        let skillsUsed = [];
        let proactiveHints = [];
        if (chatEnhancer) {
            try {
                const enhancedResponse = await chatEnhancer.processMessage(messageContent, {
                    sessionId,
                    userId: body.userId || 'anonymous',
                    tenantId: 'default',
                    conversationHistory: [],
                });
                // If skills were used, include their output in the context
                if (enhancedResponse.skillsUsed && enhancedResponse.skillsUsed.length > 0) {
                    skillsUsed = enhancedResponse.skillsUsed;
                    if (enhancedResponse.content) {
                        enhancedContext = `\n\n**Skill Results:**\n${enhancedResponse.content}\n\n`;
                    }
                }
                // Collect proactive hints
                if (enhancedResponse.proactiveHints && enhancedResponse.proactiveHints.length > 0) {
                    proactiveHints = enhancedResponse.proactiveHints;
                }
                // Log skill usage
                if (skillsUsed.length > 0) {
                    logger.info({
                        sessionId,
                        skills: skillsUsed.map(s => s.skillId),
                        memoriesRecalled: enhancedResponse.memoriesRecalled?.length || 0,
                    }, 'Skills executed');
                }
            }
            catch (enhanceError) {
                logger.warn({ err: enhanceError }, 'ChatEnhancer processing failed, continuing with standard chat');
            }
        }
        // Step 2: Get LLM response
        // Note: enhancedContext is prepended to the response content, not passed to the LLM
        const response = await bot.chat(sessionId, messageContent, {
            userId: body.userId,
            metadata: body.metadata,
        });
        logger.debug({ sessionId, responseId: response.id }, 'Chat response generated');
        // Validate output with AIDefence if enabled
        if (aiDefence && response.content) {
            try {
                const outputResult = await aiDefence.validateResponse(response.content, messageContent);
                if (!outputResult.safe) {
                    logger.warn({
                        threats: outputResult.threats,
                    }, 'Threats detected in response');
                }
            }
            catch (defenceError) {
                // Log but don't fail the request if AIDefence validation fails
                logger.warn({ err: defenceError }, 'AIDefence output validation failed');
            }
        }
        // Combine skill output with LLM response
        let finalContent = response.content;
        if (enhancedContext && !finalContent.includes(enhancedContext)) {
            finalContent = enhancedContext + finalContent;
        }
        // Add proactive hints if available
        if (proactiveHints.length > 0) {
            finalContent += '\n\n---\n💡 ' + proactiveHints.join('\n💡 ');
        }
        sendJSON(res, 200, {
            ...response,
            content: finalContent,
            skillsUsed: skillsUsed.length > 0 ? skillsUsed : undefined,
            proactiveHints: proactiveHints.length > 0 ? proactiveHints : undefined,
        });
    }
    catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        logger.error({ err: error, sessionId, errorMessage: message }, 'Chat request failed');
        sendError(res, 400, message, 'CHAT_ERROR');
    }
}
// ============================================================================
// Router
// ============================================================================
const routes = [
    { method: 'GET', pattern: /^\/$/, handler: handleRoot },
    { method: 'GET', pattern: /^\/health$/, handler: handleHealth },
    { method: 'GET', pattern: /^\/ready$/, handler: handleReady },
    { method: 'GET', pattern: /^\/api\/status$/, handler: handleStatus },
    { method: 'GET', pattern: /^\/api\/models$/, handler: handleModels },
    { method: 'GET', pattern: /^\/api\/skills$/, handler: handleSkills },
    { method: 'POST', pattern: /^\/api\/agents$/, handler: handleCreateAgent },
    { method: 'GET', pattern: /^\/api\/agents$/, handler: handleListAgents },
    { method: 'POST', pattern: /^\/api\/sessions$/, handler: handleCreateSession },
    { method: 'GET', pattern: /^\/api\/sessions$/, handler: handleListSessions },
    { method: 'POST', pattern: /^\/api\/sessions\/[^/]+\/chat$/, handler: handleChat },
];
async function handleRequest(req, res) {
    const url = new node_url_1.URL(req.url || '/', `http://${req.headers.host || 'localhost'}`);
    const method = req.method || 'GET';
    // CORS headers
    res.setHeader('Access-Control-Allow-Origin', '*');
    res.setHeader('Access-Control-Allow-Methods', 'GET, POST, PUT, DELETE, OPTIONS');
    res.setHeader('Access-Control-Allow-Headers', 'Content-Type, Authorization');
    if (method === 'OPTIONS') {
        res.writeHead(204);
        res.end();
        return;
    }
    // Find matching route
    for (const route of routes) {
        if (route.method === method && route.pattern.test(url.pathname)) {
            try {
                const body = method !== 'GET' && method !== 'HEAD'
                    ? await parseBody(req)
                    : null;
                await route.handler({ req, res, url, body });
                return;
            }
            catch (error) {
                // Use 'err' key for proper pino error serialization
                const errorMessage = error instanceof Error ? error.message : String(error);
                logger.error({ err: error, path: url.pathname, errorMessage }, 'Request handler error');
                sendError(res, 500, 'Internal server error', 'INTERNAL_ERROR');
                return;
            }
        }
    }
    // 404 Not Found
    sendError(res, 404, 'Not found', 'NOT_FOUND');
}
// ============================================================================
// Server Initialization
// ============================================================================
async function initializeBot() {
    logger.info('Initializing RuvBot...');
    bot = (0, RuvBot_js_1.createRuvBot)({
        config: {
            name: process.env.BOT_NAME || 'RuvBot',
            api: {
                enabled: false, // We're handling API ourselves
                port: PORT,
                host: HOST,
                cors: true,
                rateLimit: { max: 100, timeWindow: 60000 },
                auth: { enabled: false, type: 'bearer' },
            },
            llm: {
                provider: process.env.GOOGLE_AI_API_KEY || process.env.GEMINI_API_KEY ? 'google' : 'anthropic',
                apiKey: process.env.ANTHROPIC_API_KEY || '',
                model: process.env.DEFAULT_MODEL || (process.env.GOOGLE_AI_API_KEY || process.env.GEMINI_API_KEY ? 'gemini-2.5-flash' : 'claude-3-haiku-20240307'),
                temperature: 0.7,
                maxTokens: 4096,
                streaming: true,
            },
            slack: {
                enabled: !!process.env.SLACK_BOT_TOKEN,
                botToken: process.env.SLACK_BOT_TOKEN,
                appToken: process.env.SLACK_APP_TOKEN,
                signingSecret: process.env.SLACK_SIGNING_SECRET,
                socketMode: true,
            },
            discord: {
                enabled: !!process.env.DISCORD_TOKEN,
                token: process.env.DISCORD_TOKEN,
                clientId: process.env.DISCORD_CLIENT_ID,
                guildId: process.env.DISCORD_GUILD_ID,
            },
            memory: {
                dimensions: 384,
                maxVectors: 100000,
                indexType: 'hnsw',
                efConstruction: 200,
                efSearch: 50,
                m: 16,
            },
            logging: {
                level: process.env.LOG_LEVEL || 'info',
                pretty: NODE_ENV !== 'production',
            },
        },
    });
    await bot.start();
    // Initialize ChatEnhancer with skills and memory
    chatEnhancer = (0, ChatEnhancer_js_1.createChatEnhancer)({
        enableSkills: true,
        enableMemory: true,
        enableProactiveAssistance: true,
        memorySearchThreshold: 0.5,
        memorySearchLimit: 5,
        skillConfidenceThreshold: 0.6,
        tenantId: 'default',
    });
    logger.info('ChatEnhancer initialized with skills: web-search, memory, code, summarize');
    // Initialize AIDefence if not in development
    if (NODE_ENV === 'production') {
        const aiDefenceConfig = {
            detectPromptInjection: true,
            detectJailbreak: true,
            detectPII: true,
            blockThreshold: 'medium',
            enableAuditLog: true,
        };
        aiDefence = (0, AIDefenceGuard_js_1.createAIDefenceGuard)(aiDefenceConfig);
        logger.info('AIDefence security layer enabled');
    }
    // Create default agent
    await bot.spawnAgent({
        id: 'default-agent',
        name: 'default-agent',
        model: process.env.DEFAULT_MODEL || 'claude-3-haiku-20240307',
        systemPrompt: process.env.SYSTEM_PROMPT || 'You are RuvBot, a helpful AI assistant.',
    });
    logger.info('RuvBot initialized successfully');
}
async function startServer() {
    // Initialize bot first
    await initializeBot();
    // Create HTTP server
    const server = (0, node_http_1.createServer)((req, res) => {
        handleRequest(req, res).catch((error) => {
            const errorMessage = error instanceof Error ? error.message : String(error);
            logger.error({ err: error, errorMessage }, 'Unhandled request error');
            if (!res.headersSent) {
                sendError(res, 500, 'Internal server error', 'INTERNAL_ERROR');
            }
        });
    });
    // Graceful shutdown
    const shutdown = async (signal) => {
        logger.info({ signal }, 'Received shutdown signal');
        server.close(async () => {
            logger.info('HTTP server closed');
            if (bot) {
                await bot.stop();
                logger.info('RuvBot stopped');
            }
            process.exit(0);
        });
        // Force exit after timeout
        setTimeout(() => {
            logger.error('Forced shutdown due to timeout');
            process.exit(1);
        }, 10000);
    };
    process.on('SIGTERM', () => shutdown('SIGTERM'));
    process.on('SIGINT', () => shutdown('SIGINT'));
    // Start listening
    server.listen(PORT, HOST, () => {
        logger.info({ port: PORT, host: HOST, env: NODE_ENV }, 'RuvBot server started');
    });
}
// ============================================================================
// Main Entry Point
// ============================================================================
startServer().catch((error) => {
    const errorMessage = error instanceof Error ? error.message : String(error);
    logger.error({ err: error, errorMessage }, 'Failed to start server');
    process.exit(1);
});
//# sourceMappingURL=server.js.map