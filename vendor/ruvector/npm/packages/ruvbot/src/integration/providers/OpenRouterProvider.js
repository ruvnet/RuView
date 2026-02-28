"use strict";
/**
 * OpenRouterProvider - OpenRouter Multi-Model LLM Integration
 *
 * Provides access to multiple LLM providers through OpenRouter,
 * including Qwen QwQ reasoning models, Claude, GPT-4, and more.
 */
Object.defineProperty(exports, "__esModule", { value: true });
exports.OpenRouterProvider = void 0;
exports.createOpenRouterProvider = createOpenRouterProvider;
exports.createQwQProvider = createQwQProvider;
exports.createDeepSeekR1Provider = createDeepSeekR1Provider;
// ============================================================================
// Model Info Registry
// ============================================================================
const MODEL_INFO = {
    // QwQ Reasoning Models
    'qwen/qwq-32b': {
        id: 'qwen/qwq-32b',
        name: 'Qwen QwQ 32B (Reasoning)',
        maxTokens: 16384,
        contextWindow: 32768,
    },
    'qwen/qwq-32b:free': {
        id: 'qwen/qwq-32b:free',
        name: 'Qwen QwQ 32B Free (Reasoning)',
        maxTokens: 16384,
        contextWindow: 32768,
    },
    'qwen/qwq-32b-preview': {
        id: 'qwen/qwq-32b-preview',
        name: 'Qwen QwQ 32B Preview (Reasoning)',
        maxTokens: 16384,
        contextWindow: 32768,
    },
    // Qwen Standard Models
    'qwen/qwen3-max': {
        id: 'qwen/qwen3-max',
        name: 'Qwen3 Max',
        maxTokens: 8192,
        contextWindow: 32768,
    },
    'qwen/qwen-2.5-72b-instruct': {
        id: 'qwen/qwen-2.5-72b-instruct',
        name: 'Qwen 2.5 72B Instruct',
        maxTokens: 8192,
        contextWindow: 32768,
    },
    'qwen/qwen-2.5-coder-32b-instruct': {
        id: 'qwen/qwen-2.5-coder-32b-instruct',
        name: 'Qwen 2.5 Coder 32B',
        maxTokens: 8192,
        contextWindow: 32768,
    },
    // Anthropic
    'anthropic/claude-3.5-sonnet': {
        id: 'anthropic/claude-3.5-sonnet',
        name: 'Claude 3.5 Sonnet',
        maxTokens: 8192,
        contextWindow: 200000,
    },
    'anthropic/claude-3-opus': {
        id: 'anthropic/claude-3-opus',
        name: 'Claude 3 Opus',
        maxTokens: 4096,
        contextWindow: 200000,
    },
    // OpenAI
    'openai/gpt-4o': {
        id: 'openai/gpt-4o',
        name: 'GPT-4o',
        maxTokens: 16384,
        contextWindow: 128000,
    },
    'openai/o1-preview': {
        id: 'openai/o1-preview',
        name: 'O1 Preview (Reasoning)',
        maxTokens: 32768,
        contextWindow: 128000,
    },
    'openai/o1-mini': {
        id: 'openai/o1-mini',
        name: 'O1 Mini (Reasoning)',
        maxTokens: 65536,
        contextWindow: 128000,
    },
    // DeepSeek
    'deepseek/deepseek-r1': {
        id: 'deepseek/deepseek-r1',
        name: 'DeepSeek R1 (Reasoning)',
        maxTokens: 8192,
        contextWindow: 64000,
    },
    'deepseek/deepseek-chat': {
        id: 'deepseek/deepseek-chat',
        name: 'DeepSeek Chat',
        maxTokens: 4096,
        contextWindow: 32000,
    },
    // Google Gemini 2.x
    'google/gemini-2.5-pro-preview-05-06': {
        id: 'google/gemini-2.5-pro-preview-05-06',
        name: 'Gemini 2.5 Pro Preview',
        maxTokens: 65536,
        contextWindow: 1000000,
    },
    'google/gemini-2.0-flash-001': {
        id: 'google/gemini-2.0-flash-001',
        name: 'Gemini 2.0 Flash',
        maxTokens: 8192,
        contextWindow: 1000000,
    },
    'google/gemini-2.0-flash-lite-001': {
        id: 'google/gemini-2.0-flash-lite-001',
        name: 'Gemini 2.0 Flash Lite',
        maxTokens: 8192,
        contextWindow: 1000000,
    },
    'google/gemini-2.0-flash-thinking-exp:free': {
        id: 'google/gemini-2.0-flash-thinking-exp:free',
        name: 'Gemini 2.0 Flash Thinking (Free)',
        maxTokens: 32768,
        contextWindow: 1000000,
    },
    // Google Gemini 1.5
    'google/gemini-pro-1.5': {
        id: 'google/gemini-pro-1.5',
        name: 'Gemini Pro 1.5',
        maxTokens: 8192,
        contextWindow: 1000000,
    },
    // Meta
    'meta-llama/llama-3.1-405b-instruct': {
        id: 'meta-llama/llama-3.1-405b-instruct',
        name: 'Llama 3.1 405B Instruct',
        maxTokens: 4096,
        contextWindow: 128000,
    },
};
// ============================================================================
// OpenRouterProvider Implementation
// ============================================================================
class OpenRouterProvider {
    constructor(config) {
        this.config = {
            apiKey: config.apiKey,
            baseUrl: config.baseUrl ?? 'https://openrouter.ai/api',
            model: config.model ?? 'qwen/qwq-32b', // Default to QwQ reasoning model
            siteUrl: config.siteUrl ?? '',
            siteName: config.siteName ?? 'RuvBot',
            maxRetries: config.maxRetries ?? 3,
            timeout: config.timeout ?? 120000, // Longer timeout for reasoning models
        };
        this.model = this.config.model;
    }
    /**
     * Complete a conversation
     */
    async complete(messages, options) {
        const modelInfo = this.getModel();
        const response = await this.makeRequest('/v1/chat/completions', {
            model: this.model,
            max_tokens: options?.maxTokens ?? modelInfo.maxTokens,
            temperature: options?.temperature ?? 1.0,
            top_p: options?.topP,
            stop: options?.stopSequences,
            messages: this.convertMessages(messages),
            tools: options?.tools ? this.convertTools(options.tools) : undefined,
        });
        return this.convertResponse(response);
    }
    /**
     * Stream a conversation
     */
    async *stream(messages, options) {
        const modelInfo = this.getModel();
        const response = await this.makeStreamRequest('/v1/chat/completions', {
            model: this.model,
            max_tokens: options?.maxTokens ?? modelInfo.maxTokens,
            temperature: options?.temperature ?? 1.0,
            top_p: options?.topP,
            stop: options?.stopSequences,
            messages: this.convertMessages(messages),
            tools: options?.tools ? this.convertTools(options.tools) : undefined,
            stream: true,
        });
        let fullContent = '';
        let inputTokens = 0;
        let outputTokens = 0;
        const toolCalls = [];
        let finishReason = 'stop';
        const pendingToolCalls = new Map();
        for await (const chunk of response) {
            const choice = chunk.choices[0];
            if (!choice)
                continue;
            // Handle content delta
            if (choice.delta.content) {
                fullContent += choice.delta.content;
                options?.onToken?.(choice.delta.content);
                yield { type: 'text', text: choice.delta.content };
            }
            // Handle tool calls
            if (choice.delta.tool_calls) {
                for (const tc of choice.delta.tool_calls) {
                    if (!pendingToolCalls.has(tc.index)) {
                        pendingToolCalls.set(tc.index, { id: tc.id ?? '', name: '', arguments: '' });
                    }
                    const pending = pendingToolCalls.get(tc.index);
                    if (tc.id)
                        pending.id = tc.id;
                    if (tc.function?.name)
                        pending.name = tc.function.name;
                    if (tc.function?.arguments)
                        pending.arguments += tc.function.arguments;
                }
            }
            // Handle finish reason
            if (choice.finish_reason) {
                if (choice.finish_reason === 'tool_calls')
                    finishReason = 'tool_use';
                else if (choice.finish_reason === 'length')
                    finishReason = 'length';
            }
            // Handle usage
            if (chunk.usage) {
                inputTokens = chunk.usage.prompt_tokens;
                outputTokens = chunk.usage.completion_tokens;
            }
        }
        // Finalize tool calls
        for (const pending of pendingToolCalls.values()) {
            if (pending.id && pending.name) {
                try {
                    const input = JSON.parse(pending.arguments || '{}');
                    toolCalls.push({ id: pending.id, name: pending.name, input });
                    yield { type: 'tool_use', toolUse: { id: pending.id, name: pending.name, input } };
                }
                catch {
                    // Skip invalid JSON
                }
            }
        }
        return {
            content: fullContent,
            finishReason,
            usage: { inputTokens, outputTokens },
            toolCalls: toolCalls.length > 0 ? toolCalls : undefined,
        };
    }
    /**
     * Count tokens in text
     */
    async countTokens(text) {
        // Approximate token count (~4 chars per token)
        return Math.ceil(text.length / 4);
    }
    /**
     * Get model info
     */
    getModel() {
        return MODEL_INFO[this.model] ?? {
            id: this.model,
            name: this.model,
            maxTokens: 4096,
            contextWindow: 32000,
        };
    }
    /**
     * Check provider health
     */
    async isHealthy() {
        try {
            const response = await fetch(`${this.config.baseUrl}/v1/models`, {
                headers: {
                    'Authorization': `Bearer ${this.config.apiKey}`,
                },
            });
            return response.ok;
        }
        catch {
            return false;
        }
    }
    /**
     * List available models
     */
    async listModels() {
        try {
            const response = await fetch(`${this.config.baseUrl}/v1/models`, {
                headers: {
                    'Authorization': `Bearer ${this.config.apiKey}`,
                },
            });
            if (!response.ok)
                return Object.keys(MODEL_INFO);
            const data = await response.json();
            return data.data.map(m => m.id);
        }
        catch {
            return Object.keys(MODEL_INFO);
        }
    }
    // ==========================================================================
    // Private Methods
    // ==========================================================================
    convertMessages(messages) {
        return messages.map(msg => ({
            role: msg.role,
            content: msg.content,
        }));
    }
    convertTools(tools) {
        return tools.map(tool => ({
            type: 'function',
            function: {
                name: tool.name,
                description: tool.description,
                parameters: tool.parameters,
            },
        }));
    }
    convertResponse(response) {
        const choice = response.choices[0];
        const toolCalls = (choice.message.tool_calls ?? []).map(tc => ({
            id: tc.id,
            name: tc.function.name,
            input: JSON.parse(tc.function.arguments || '{}'),
        }));
        let finishReason = 'stop';
        if (choice.finish_reason === 'length')
            finishReason = 'length';
        if (choice.finish_reason === 'tool_calls')
            finishReason = 'tool_use';
        return {
            content: choice.message.content ?? '',
            finishReason,
            usage: {
                inputTokens: response.usage.prompt_tokens,
                outputTokens: response.usage.completion_tokens,
            },
            toolCalls: toolCalls.length > 0 ? toolCalls : undefined,
        };
    }
    async makeRequest(endpoint, body) {
        const url = `${this.config.baseUrl}${endpoint}`;
        const headers = {
            'Content-Type': 'application/json',
            'Authorization': `Bearer ${this.config.apiKey}`,
        };
        if (this.config.siteUrl) {
            headers['HTTP-Referer'] = this.config.siteUrl;
        }
        if (this.config.siteName) {
            headers['X-Title'] = this.config.siteName;
        }
        const response = await fetch(url, {
            method: 'POST',
            headers,
            body: JSON.stringify(body),
            signal: AbortSignal.timeout(this.config.timeout),
        });
        if (!response.ok) {
            const error = await response.text();
            throw new Error(`OpenRouter API error: ${response.status} - ${error}`);
        }
        return response.json();
    }
    async *makeStreamRequest(endpoint, body) {
        const url = `${this.config.baseUrl}${endpoint}`;
        const headers = {
            'Content-Type': 'application/json',
            'Authorization': `Bearer ${this.config.apiKey}`,
        };
        if (this.config.siteUrl) {
            headers['HTTP-Referer'] = this.config.siteUrl;
        }
        if (this.config.siteName) {
            headers['X-Title'] = this.config.siteName;
        }
        const response = await fetch(url, {
            method: 'POST',
            headers,
            body: JSON.stringify(body),
        });
        if (!response.ok) {
            const error = await response.text();
            throw new Error(`OpenRouter API error: ${response.status} - ${error}`);
        }
        const reader = response.body?.getReader();
        if (!reader) {
            throw new Error('No response body');
        }
        const decoder = new TextDecoder();
        let buffer = '';
        try {
            while (true) {
                const { done, value } = await reader.read();
                if (done)
                    break;
                buffer += decoder.decode(value, { stream: true });
                const lines = buffer.split('\n');
                buffer = lines.pop() ?? '';
                for (const line of lines) {
                    if (line.startsWith('data: ')) {
                        const data = line.slice(6);
                        if (data === '[DONE]')
                            return;
                        try {
                            yield JSON.parse(data);
                        }
                        catch {
                            // Skip invalid JSON
                        }
                    }
                }
            }
        }
        finally {
            reader.releaseLock();
        }
    }
}
exports.OpenRouterProvider = OpenRouterProvider;
// ============================================================================
// Factory Functions
// ============================================================================
function createOpenRouterProvider(config) {
    return new OpenRouterProvider(config);
}
/**
 * Create a provider for Qwen QwQ reasoning model
 */
function createQwQProvider(apiKey, free = false) {
    return new OpenRouterProvider({
        apiKey,
        model: free ? 'qwen/qwq-32b:free' : 'qwen/qwq-32b',
    });
}
/**
 * Create a provider for DeepSeek R1 reasoning model
 */
function createDeepSeekR1Provider(apiKey) {
    return new OpenRouterProvider({
        apiKey,
        model: 'deepseek/deepseek-r1',
    });
}
exports.default = OpenRouterProvider;
//# sourceMappingURL=OpenRouterProvider.js.map