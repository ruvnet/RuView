"use strict";
/**
 * OpenRouter API client with Kimi K2 support
 */
var __importDefault = (this && this.__importDefault) || function (mod) {
    return (mod && mod.__esModule) ? mod : { "default": mod };
};
Object.defineProperty(exports, "__esModule", { value: true });
exports.OpenRouterClient = void 0;
exports.createOpenRouterClient = createOpenRouterClient;
const p_retry_1 = __importDefault(require("p-retry"));
const p_throttle_1 = __importDefault(require("p-throttle"));
const types_js_1 = require("./types.js");
class OpenRouterClient {
    constructor(config = {}) {
        const apiKey = config.apiKey || process.env.OPENROUTER_API_KEY;
        if (!apiKey) {
            throw new Error('OpenRouter API key is required. Set OPENROUTER_API_KEY environment variable or pass apiKey in config.');
        }
        this.config = types_js_1.OpenRouterConfigSchema.parse({ ...config, apiKey });
        // Setup rate limiting if configured
        if (this.config.rateLimit) {
            this.throttledFetch = (0, p_throttle_1.default)({
                limit: this.config.rateLimit.requests,
                interval: this.config.rateLimit.interval
            })(fetch.bind(globalThis));
        }
        else {
            this.throttledFetch = fetch.bind(globalThis);
        }
    }
    /**
     * Create a chat completion
     */
    async createCompletion(messages, options = {}) {
        const request = {
            model: this.config.model || 'moonshot/kimi-k2-instruct',
            messages,
            temperature: options.temperature ?? 0.7,
            max_tokens: options.max_tokens ?? 4096,
            top_p: options.top_p ?? 1,
            stream: false,
            ...options
        };
        return (0, p_retry_1.default)(async () => {
            const response = await this.throttledFetch(`${this.config.baseURL}/chat/completions`, {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json',
                    'Authorization': `Bearer ${this.config.apiKey}`,
                    'HTTP-Referer': 'https://github.com/ruvnet/ruvector',
                    'X-Title': 'RuVector Graph Data Generator'
                },
                body: JSON.stringify(request),
                signal: AbortSignal.timeout(this.config.timeout || 60000)
            });
            if (!response.ok) {
                const error = await response.text();
                throw new types_js_1.OpenRouterError(`OpenRouter API error: ${response.status} ${response.statusText}`, { status: response.status, error });
            }
            const data = await response.json();
            return data;
        }, {
            retries: this.config.maxRetries || 3,
            onFailedAttempt: (error) => {
                console.warn(`Attempt ${error.attemptNumber} failed. ${error.retriesLeft} retries left.`);
            }
        });
    }
    /**
     * Create a streaming chat completion
     */
    async *createStreamingCompletion(messages, options = {}) {
        const request = {
            model: this.config.model || 'moonshot/kimi-k2-instruct',
            messages,
            temperature: options.temperature ?? 0.7,
            max_tokens: options.max_tokens ?? 4096,
            top_p: options.top_p ?? 1,
            stream: true,
            ...options
        };
        const response = await this.throttledFetch(`${this.config.baseURL}/chat/completions`, {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
                'Authorization': `Bearer ${this.config.apiKey}`,
                'HTTP-Referer': 'https://github.com/ruvnet/ruvector',
                'X-Title': 'RuVector Graph Data Generator'
            },
            body: JSON.stringify(request),
            signal: AbortSignal.timeout(this.config.timeout || 60000)
        });
        if (!response.ok) {
            const error = await response.text();
            throw new types_js_1.OpenRouterError(`OpenRouter API error: ${response.status} ${response.statusText}`, { status: response.status, error });
        }
        if (!response.body) {
            throw new types_js_1.OpenRouterError('No response body received');
        }
        const reader = response.body.getReader();
        const decoder = new TextDecoder();
        let buffer = '';
        try {
            while (true) {
                const { done, value } = await reader.read();
                if (done)
                    break;
                buffer += decoder.decode(value, { stream: true });
                const lines = buffer.split('\n');
                buffer = lines.pop() || '';
                for (const line of lines) {
                    if (line.startsWith('data: ')) {
                        const data = line.slice(6).trim();
                        if (data === '[DONE]') {
                            return;
                        }
                        try {
                            const parsed = JSON.parse(data);
                            const content = parsed.choices?.[0]?.delta?.content;
                            if (content) {
                                yield content;
                            }
                        }
                        catch (e) {
                            console.warn('Failed to parse SSE data:', data);
                        }
                    }
                }
            }
        }
        finally {
            reader.releaseLock();
        }
    }
    /**
     * Generate structured data using prompt engineering
     */
    async generateStructured(systemPrompt, userPrompt, options) {
        const messages = [
            { role: 'system', content: systemPrompt },
            { role: 'user', content: userPrompt }
        ];
        const response = await this.createCompletion(messages, {
            temperature: options?.temperature ?? 0.7,
            max_tokens: options?.maxTokens ?? 4096
        });
        const content = response.choices[0]?.message?.content;
        if (!content) {
            throw new types_js_1.OpenRouterError('No content in response');
        }
        // Try to extract JSON from response
        try {
            // Look for JSON in code blocks
            const jsonMatch = content.match(/```(?:json)?\s*(\{[\s\S]*?\}|\[[\s\S]*?\])\s*```/);
            if (jsonMatch) {
                return JSON.parse(jsonMatch[1]);
            }
            // Try to parse the entire response as JSON
            return JSON.parse(content);
        }
        catch (e) {
            throw new types_js_1.OpenRouterError('Failed to parse JSON from response', { content, error: e });
        }
    }
    /**
     * Generate embeddings (if the model supports it)
     */
    async generateEmbedding(_text) {
        // Note: Kimi K2 may not support embeddings directly
        // This is a placeholder for potential future support
        throw new Error('Embedding generation not yet implemented for Kimi K2');
    }
    /**
     * Update configuration
     */
    configure(config) {
        this.config = types_js_1.OpenRouterConfigSchema.parse({ ...this.config, ...config });
    }
    /**
     * Get current configuration
     */
    getConfig() {
        return { ...this.config };
    }
}
exports.OpenRouterClient = OpenRouterClient;
/**
 * Create a new OpenRouter client
 */
function createOpenRouterClient(config) {
    return new OpenRouterClient(config);
}
//# sourceMappingURL=openrouter-client.js.map