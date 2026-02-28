"use strict";
/**
 * AnthropicProvider - Anthropic Claude LLM Integration
 *
 * Provides Claude AI models (claude-3-opus, claude-3-sonnet, claude-3-haiku)
 * with streaming, tool use, and full API support.
 */
Object.defineProperty(exports, "__esModule", { value: true });
exports.AnthropicProvider = void 0;
exports.createAnthropicProvider = createAnthropicProvider;
// ============================================================================
// Model Info
// ============================================================================
const MODEL_INFO = {
    'claude-opus-4-20250514': {
        id: 'claude-opus-4-20250514',
        name: 'Claude Opus 4',
        maxTokens: 32768,
        contextWindow: 200000,
    },
    'claude-sonnet-4-20250514': {
        id: 'claude-sonnet-4-20250514',
        name: 'Claude Sonnet 4',
        maxTokens: 16384,
        contextWindow: 200000,
    },
    'claude-3-5-sonnet-20241022': {
        id: 'claude-3-5-sonnet-20241022',
        name: 'Claude 3.5 Sonnet',
        maxTokens: 8192,
        contextWindow: 200000,
    },
    'claude-3-5-haiku-20241022': {
        id: 'claude-3-5-haiku-20241022',
        name: 'Claude 3.5 Haiku',
        maxTokens: 8192,
        contextWindow: 200000,
    },
    'claude-3-opus-20240229': {
        id: 'claude-3-opus-20240229',
        name: 'Claude 3 Opus',
        maxTokens: 4096,
        contextWindow: 200000,
    },
    'claude-3-sonnet-20240229': {
        id: 'claude-3-sonnet-20240229',
        name: 'Claude 3 Sonnet',
        maxTokens: 4096,
        contextWindow: 200000,
    },
    'claude-3-haiku-20240307': {
        id: 'claude-3-haiku-20240307',
        name: 'Claude 3 Haiku',
        maxTokens: 4096,
        contextWindow: 200000,
    },
};
// ============================================================================
// AnthropicProvider Implementation
// ============================================================================
class AnthropicProvider {
    constructor(config) {
        this.config = {
            apiKey: config.apiKey,
            baseUrl: config.baseUrl ?? 'https://api.anthropic.com',
            model: config.model ?? 'claude-3-5-sonnet-20241022',
            maxRetries: config.maxRetries ?? 3,
            timeout: config.timeout ?? 60000,
        };
        this.model = this.config.model;
    }
    /**
     * Complete a conversation
     */
    async complete(messages, options) {
        const response = await this.makeRequest('/v1/messages', {
            model: this.model,
            max_tokens: options?.maxTokens ?? MODEL_INFO[this.model].maxTokens,
            temperature: options?.temperature ?? 1.0,
            top_p: options?.topP,
            stop_sequences: options?.stopSequences,
            messages: this.convertMessages(messages),
            tools: options?.tools ? this.convertTools(options.tools) : undefined,
        });
        return this.convertResponse(response);
    }
    /**
     * Stream a conversation
     */
    async *stream(messages, options) {
        const response = await this.makeStreamRequest('/v1/messages', {
            model: this.model,
            max_tokens: options?.maxTokens ?? MODEL_INFO[this.model].maxTokens,
            temperature: options?.temperature ?? 1.0,
            top_p: options?.topP,
            stop_sequences: options?.stopSequences,
            messages: this.convertMessages(messages),
            tools: options?.tools ? this.convertTools(options.tools) : undefined,
            stream: true,
        });
        let fullContent = '';
        let inputTokens = 0;
        let outputTokens = 0;
        const toolCalls = [];
        let finishReason = 'stop';
        for await (const event of response) {
            if (event.type === 'content_block_delta' && event.delta?.text) {
                fullContent += event.delta.text;
                options?.onToken?.(event.delta.text);
                yield { type: 'text', text: event.delta.text };
            }
            else if (event.type === 'message_delta') {
                if (event.usage?.output_tokens) {
                    outputTokens = event.usage.output_tokens;
                }
                if (event.message?.stop_reason === 'tool_use') {
                    finishReason = 'tool_use';
                }
            }
            else if (event.type === 'message_start' && event.message?.usage) {
                inputTokens = event.message.usage.input_tokens;
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
        // Approximate token count (Claude uses ~4 chars per token on average)
        return Math.ceil(text.length / 4);
    }
    /**
     * Get model info
     */
    getModel() {
        return MODEL_INFO[this.model] ?? MODEL_INFO['claude-3-5-sonnet-20241022'];
    }
    /**
     * Check provider health
     */
    async isHealthy() {
        try {
            // Simple health check - try to count tokens
            await this.countTokens('health check');
            return true;
        }
        catch {
            return false;
        }
    }
    // ==========================================================================
    // Private Methods
    // ==========================================================================
    convertMessages(messages) {
        const anthropicMessages = [];
        let systemPrompt = '';
        for (const msg of messages) {
            if (msg.role === 'system') {
                systemPrompt += (systemPrompt ? '\n' : '') + msg.content;
            }
            else {
                anthropicMessages.push({
                    role: msg.role,
                    content: msg.content,
                });
            }
        }
        // Prepend system prompt to first user message if exists
        if (systemPrompt && anthropicMessages.length > 0 && anthropicMessages[0].role === 'user') {
            const firstContent = anthropicMessages[0].content;
            if (typeof firstContent === 'string') {
                anthropicMessages[0].content = `${systemPrompt}\n\n${firstContent}`;
            }
        }
        return anthropicMessages;
    }
    convertTools(tools) {
        return tools.map(tool => ({
            name: tool.name,
            description: tool.description,
            input_schema: tool.parameters,
        }));
    }
    convertResponse(response) {
        let content = '';
        const toolCalls = [];
        for (const block of response.content) {
            if (block.type === 'text' && block.text) {
                content += block.text;
            }
            else if (block.type === 'tool_use' && block.id && block.name) {
                toolCalls.push({
                    id: block.id,
                    name: block.name,
                    input: block.input ?? {},
                });
            }
        }
        let finishReason = 'stop';
        if (response.stop_reason === 'max_tokens')
            finishReason = 'length';
        if (response.stop_reason === 'tool_use')
            finishReason = 'tool_use';
        return {
            content,
            finishReason,
            usage: {
                inputTokens: response.usage.input_tokens,
                outputTokens: response.usage.output_tokens,
            },
            toolCalls: toolCalls.length > 0 ? toolCalls : undefined,
        };
    }
    async makeRequest(endpoint, body) {
        const url = `${this.config.baseUrl}${endpoint}`;
        const response = await fetch(url, {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
                'x-api-key': this.config.apiKey,
                'anthropic-version': '2023-06-01',
            },
            body: JSON.stringify(body),
            signal: AbortSignal.timeout(this.config.timeout),
        });
        if (!response.ok) {
            const error = await response.text();
            throw new Error(`Anthropic API error: ${response.status} - ${error}`);
        }
        return response.json();
    }
    async *makeStreamRequest(endpoint, body) {
        const url = `${this.config.baseUrl}${endpoint}`;
        const response = await fetch(url, {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
                'x-api-key': this.config.apiKey,
                'anthropic-version': '2023-06-01',
            },
            body: JSON.stringify(body),
        });
        if (!response.ok) {
            const error = await response.text();
            throw new Error(`Anthropic API error: ${response.status} - ${error}`);
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
exports.AnthropicProvider = AnthropicProvider;
// ============================================================================
// Factory Function
// ============================================================================
function createAnthropicProvider(config) {
    return new AnthropicProvider(config);
}
exports.default = AnthropicProvider;
//# sourceMappingURL=AnthropicProvider.js.map