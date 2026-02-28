"use strict";
/**
 * GoogleAIProvider - Google AI (Gemini) LLM Integration
 *
 * Provides direct access to Google's Gemini models using the Google AI API.
 * Supports both API key authentication and Google Cloud default credentials.
 */
Object.defineProperty(exports, "__esModule", { value: true });
exports.GoogleAIProvider = void 0;
exports.createGoogleAIProvider = createGoogleAIProvider;
// ============================================================================
// Model Info
// ============================================================================
const MODEL_INFO = {
    // Gemini 3.x models (preview)
    'gemini-3-pro-preview': {
        id: 'gemini-3-pro-preview',
        name: 'Gemini 3 Pro Preview',
        maxTokens: 65536,
        contextWindow: 1000000,
    },
    'gemini-3-flash-preview': {
        id: 'gemini-3-flash-preview',
        name: 'Gemini 3 Flash Preview',
        maxTokens: 65536,
        contextWindow: 1000000,
    },
    // Gemini 2.5 models (stable)
    'gemini-2.5-pro': {
        id: 'gemini-2.5-pro',
        name: 'Gemini 2.5 Pro',
        maxTokens: 65536,
        contextWindow: 1000000,
    },
    'gemini-2.5-flash': {
        id: 'gemini-2.5-flash',
        name: 'Gemini 2.5 Flash',
        maxTokens: 65536,
        contextWindow: 1000000,
    },
    'gemini-2.5-flash-lite': {
        id: 'gemini-2.5-flash-lite',
        name: 'Gemini 2.5 Flash Lite',
        maxTokens: 65536,
        contextWindow: 1000000,
    },
    'gemini-2.5-flash-image': {
        id: 'gemini-2.5-flash-image',
        name: 'Gemini 2.5 Flash Image',
        maxTokens: 65536,
        contextWindow: 1000000,
    },
    // Gemini 2.0 models (deprecated March 2026)
    'gemini-2.0-flash': {
        id: 'gemini-2.0-flash',
        name: 'Gemini 2.0 Flash',
        maxTokens: 8192,
        contextWindow: 1000000,
    },
    'gemini-2.0-flash-lite': {
        id: 'gemini-2.0-flash-lite',
        name: 'Gemini 2.0 Flash Lite',
        maxTokens: 8192,
        contextWindow: 1000000,
    },
    // Gemini 1.5 models
    'gemini-1.5-pro': {
        id: 'gemini-1.5-pro',
        name: 'Gemini 1.5 Pro',
        maxTokens: 8192,
        contextWindow: 2000000,
    },
    'gemini-1.5-flash': {
        id: 'gemini-1.5-flash',
        name: 'Gemini 1.5 Flash',
        maxTokens: 8192,
        contextWindow: 1000000,
    },
    'gemini-1.5-flash-8b': {
        id: 'gemini-1.5-flash-8b',
        name: 'Gemini 1.5 Flash 8B',
        maxTokens: 8192,
        contextWindow: 1000000,
    },
};
// ============================================================================
// Provider Implementation
// ============================================================================
class GoogleAIProvider {
    constructor(config) {
        this.config = {
            apiKey: config.apiKey || process.env.GOOGLE_AI_API_KEY || process.env.GEMINI_API_KEY || '',
            projectId: config.projectId || process.env.GOOGLE_CLOUD_PROJECT || '',
            location: config.location || 'us-central1',
            model: config.model || 'gemini-2.0-flash',
            maxRetries: config.maxRetries ?? 3,
            timeout: config.timeout ?? 60000,
        };
        this.modelId = this.config.model;
        this.baseUrl = 'https://generativelanguage.googleapis.com/v1beta';
    }
    async complete(messages, options) {
        const geminiMessages = this.convertMessages(messages);
        const systemInstruction = this.extractSystemInstruction(messages);
        const requestBody = {
            contents: geminiMessages,
            generationConfig: {
                temperature: options?.temperature ?? 0.7,
                maxOutputTokens: options?.maxTokens ?? 4096,
                topP: options?.topP ?? 0.95,
            },
        };
        if (systemInstruction) {
            requestBody.systemInstruction = {
                parts: [{ text: systemInstruction }],
            };
        }
        if (options?.tools && options.tools.length > 0) {
            requestBody.tools = [{
                    functionDeclarations: options.tools.map(tool => ({
                        name: tool.name,
                        description: tool.description,
                        parameters: tool.parameters,
                    })),
                }];
        }
        const url = `${this.baseUrl}/models/${this.modelId}:generateContent?key=${this.config.apiKey}`;
        let lastError = null;
        for (let attempt = 0; attempt < this.config.maxRetries; attempt++) {
            try {
                const response = await fetch(url, {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify(requestBody),
                    signal: AbortSignal.timeout(this.config.timeout),
                });
                if (!response.ok) {
                    const errorText = await response.text();
                    throw new Error(`Google AI API error (${response.status}): ${errorText}`);
                }
                const data = await response.json();
                if (!data.candidates || data.candidates.length === 0) {
                    throw new Error('No response from Google AI');
                }
                const candidate = data.candidates[0];
                const content = candidate.content.parts.map(p => p.text).join('');
                const finishReason = this.mapFinishReason(candidate.finishReason);
                return {
                    content,
                    finishReason,
                    usage: {
                        inputTokens: data.usageMetadata?.promptTokenCount ?? 0,
                        outputTokens: data.usageMetadata?.candidatesTokenCount ?? 0,
                    },
                };
            }
            catch (error) {
                lastError = error;
                if (attempt < this.config.maxRetries - 1) {
                    await new Promise(resolve => setTimeout(resolve, 1000 * (attempt + 1)));
                }
            }
        }
        throw lastError || new Error('Failed to complete request');
    }
    async *stream(messages, options) {
        const geminiMessages = this.convertMessages(messages);
        const systemInstruction = this.extractSystemInstruction(messages);
        const requestBody = {
            contents: geminiMessages,
            generationConfig: {
                temperature: options?.temperature ?? 0.7,
                maxOutputTokens: options?.maxTokens ?? 4096,
            },
        };
        if (systemInstruction) {
            requestBody.systemInstruction = {
                parts: [{ text: systemInstruction }],
            };
        }
        const url = `${this.baseUrl}/models/${this.modelId}:streamGenerateContent?key=${this.config.apiKey}&alt=sse`;
        const response = await fetch(url, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(requestBody),
        });
        if (!response.ok) {
            const errorText = await response.text();
            throw new Error(`Google AI API error (${response.status}): ${errorText}`);
        }
        const reader = response.body?.getReader();
        if (!reader) {
            throw new Error('No response body');
        }
        const decoder = new TextDecoder();
        let buffer = '';
        let fullContent = '';
        let inputTokens = 0;
        let outputTokens = 0;
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
                        const data = line.slice(6);
                        if (data === '[DONE]')
                            continue;
                        try {
                            const json = JSON.parse(data);
                            if (json.candidates?.[0]?.content?.parts?.[0]?.text) {
                                const text = json.candidates[0].content.parts[0].text;
                                fullContent += text;
                                options?.onToken?.(text);
                                yield { type: 'text', text };
                            }
                            if (json.usageMetadata) {
                                inputTokens = json.usageMetadata.promptTokenCount;
                                outputTokens = json.usageMetadata.candidatesTokenCount;
                            }
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
        return {
            content: fullContent,
            finishReason: 'stop',
            usage: { inputTokens, outputTokens },
        };
    }
    async countTokens(text) {
        // Approximate token count (Gemini uses ~4 chars per token on average)
        return Math.ceil(text.length / 4);
    }
    getModel() {
        return MODEL_INFO[this.modelId] ?? {
            id: this.modelId,
            name: this.modelId,
            maxTokens: 8192,
            contextWindow: 1000000,
        };
    }
    async isHealthy() {
        try {
            await this.countTokens('health check');
            return !!this.config.apiKey;
        }
        catch {
            return false;
        }
    }
    convertMessages(messages) {
        return messages
            .filter(m => m.role !== 'system')
            .map(m => ({
            role: m.role === 'assistant' ? 'model' : 'user',
            parts: [{ text: m.content }],
        }));
    }
    extractSystemInstruction(messages) {
        const systemMessage = messages.find(m => m.role === 'system');
        return systemMessage?.content || null;
    }
    mapFinishReason(reason) {
        switch (reason) {
            case 'STOP':
            case 'END_TURN':
                return 'stop';
            case 'MAX_TOKENS':
                return 'length';
            case 'TOOL_CALL':
                return 'tool_use';
            default:
                return 'stop';
        }
    }
}
exports.GoogleAIProvider = GoogleAIProvider;
// ============================================================================
// Factory Function
// ============================================================================
function createGoogleAIProvider(config) {
    return new GoogleAIProvider(config);
}
//# sourceMappingURL=GoogleAIProvider.js.map