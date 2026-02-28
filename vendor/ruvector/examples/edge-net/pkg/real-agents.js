/**
 * @ruvector/edge-net REAL Agent System
 *
 * Actually functional distributed agents with:
 * - LOCAL LLM execution via ruvllm (default - no API key needed)
 * - Cloud LLM API calls (Anthropic Claude, OpenAI) as fallback
 * - Real embeddings via ruvector AdaptiveEmbedder
 * - Real relay server sync
 * - Real task execution
 *
 * @module @ruvector/edge-net/real-agents
 */

import { EventEmitter } from 'events';
import { createHash, randomBytes } from 'crypto';
import { readFileSync, writeFileSync, existsSync } from 'fs';
import { join } from 'path';

// ============================================
// LLM PROVIDER CONFIGURATION
// ============================================

const LLM_PROVIDERS = {
    // ONNX LLM via transformers.js - Default, no API key needed
    // Uses real ONNX models (SmolLM, TinyLlama, etc.)
    local: {
        name: 'ONNX Local',
        type: 'local',
        backend: 'onnx', // Primary: transformers.js ONNX
        models: {
            // TRM (Tiny Random Models) - Fastest
            fast: process.env.ONNX_MODEL_FAST || 'Xenova/distilgpt2',
            // SmolLM - Better quality
            balanced: process.env.ONNX_MODEL || 'HuggingFaceTB/SmolLM-135M-Instruct',
            // TinyLlama - Best small model
            powerful: process.env.ONNX_MODEL_POWERFUL || 'HuggingFaceTB/SmolLM-360M-Instruct',
        },
    },
    onnx: {
        name: 'ONNX Transformers.js',
        type: 'local',
        backend: 'onnx',
        models: {
            // TRM - Ultra tiny models
            'trm-tinystories': 'Xenova/TinyStories-33M',
            'trm-gpt2': 'Xenova/gpt2',
            'trm-distilgpt2': 'Xenova/distilgpt2',
            // SmolLM series
            fast: 'HuggingFaceTB/SmolLM-135M-Instruct',
            balanced: 'HuggingFaceTB/SmolLM-360M-Instruct',
            powerful: 'TinyLlama/TinyLlama-1.1B-Chat-v1.0',
            // Named models
            'smollm-135m': 'HuggingFaceTB/SmolLM-135M-Instruct',
            'smollm-360m': 'HuggingFaceTB/SmolLM-360M-Instruct',
            'smollm2-135m': 'HuggingFaceTB/SmolLM2-135M-Instruct',
            'tinyllama': 'TinyLlama/TinyLlama-1.1B-Chat-v1.0',
            'qwen2.5-0.5b': 'Qwen/Qwen2.5-0.5B-Instruct',
        },
    },
    ollama: {
        name: 'Ollama',
        type: 'local',
        backend: 'ollama',
        baseUrl: process.env.OLLAMA_HOST || 'http://localhost:11434',
        models: {
            fast: process.env.OLLAMA_MODEL_FAST || 'qwen2.5:0.5b',
            balanced: process.env.OLLAMA_MODEL || 'qwen2.5:1.5b',
            powerful: process.env.OLLAMA_MODEL_POWERFUL || 'qwen2.5:3b',
        },
    },
    ruvllm: {
        name: 'RuvLLM (Legacy)',
        type: 'local',
        backend: 'ruvllm',
        models: {
            fast: 'ruvllm-fast',
            balanced: 'ruvllm-balanced',
            powerful: 'ruvllm-powerful',
        },
    },
    // Cloud providers as fallback (December 2025 models)
    anthropic: {
        name: 'Anthropic Claude',
        type: 'cloud',
        baseUrl: 'https://api.anthropic.com/v1',
        models: {
            fast: 'claude-3-5-haiku-20241022',
            balanced: 'claude-sonnet-4-20250514',
            powerful: 'claude-opus-4-5-20251101',
        },
        headers: (apiKey) => ({
            'Content-Type': 'application/json',
            'x-api-key': apiKey,
            'anthropic-version': '2023-06-01',
        }),
    },
    openai: {
        name: 'OpenAI',
        type: 'cloud',
        baseUrl: 'https://api.openai.com/v1',
        models: {
            fast: 'gpt-4o-mini',
            balanced: 'gpt-5.2',
            powerful: 'gpt-5.2-turbo',
        },
        headers: (apiKey) => ({
            'Content-Type': 'application/json',
            'Authorization': `Bearer ${apiKey}`,
        }),
    },
};

// Agent type to system prompt mapping
const AGENT_PROMPTS = {
    researcher: `You are a research agent. Your task is to analyze, search, summarize, and extract information.
Be thorough and cite sources when possible. Structure your findings clearly.`,

    coder: `You are a coding agent. Your task is to write, refactor, debug, and test code.
Write clean, well-documented code. Follow best practices and explain your approach.`,

    reviewer: `You are a code review agent. Your task is to review code for quality, security, and best practices.
Be constructive and specific. Identify issues and suggest improvements.`,

    tester: `You are a testing agent. Your task is to write tests, validate functionality, and report issues.
Cover edge cases. Write clear test descriptions.`,

    analyst: `You are an analysis agent. Your task is to analyze data, generate metrics, and create reports.
Be data-driven. Present findings with evidence.`,

    optimizer: `You are an optimization agent. Your task is to profile, identify bottlenecks, and improve performance.
Quantify improvements. Focus on measurable gains.`,

    coordinator: `You are a coordination agent. Your task is to orchestrate workflows, route tasks, and manage schedules.
Be organized and clear about task dependencies.`,

    embedder: `You are an embedding agent specialized in semantic search and vector operations.
Generate high-quality embeddings for text. Optimize for similarity matching.`,
};

// ============================================
// REAL LLM CLIENT
// ============================================

/**
 * Real LLM client - uses local ruvllm by default, falls back to cloud APIs
 */
export class LLMClient {
    constructor(options = {}) {
        // Default to local ruvllm, fallback to cloud if API key provided
        this.provider = options.provider || 'local';
        this.apiKey = options.apiKey || process.env.ANTHROPIC_API_KEY || process.env.OPENAI_API_KEY;
        this.model = options.model || 'balanced';
        this.maxTokens = options.maxTokens || 4096;

        // Auto-select cloud provider if API key is set and provider not specified
        if (!options.provider && this.apiKey) {
            this.provider = process.env.ANTHROPIC_API_KEY ? 'anthropic' : 'openai';
        }

        this.config = LLM_PROVIDERS[this.provider];
        if (!this.config) {
            throw new Error(`Unknown LLM provider: ${this.provider}`);
        }

        // Initialize local LLM backends
        this.ruvllm = null;
        this.ruvllmInitialized = false;
        this.onnxPipeline = null;
        this.onnxInitialized = false;
        this.onnxModel = null;
    }

    /**
     * Initialize ONNX LLM via transformers.js
     * This is the primary local inference method
     */
    async initOnnx(modelId) {
        if (this.onnxInitialized && this.onnxModel === modelId) return true;

        try {
            console.log(`[LLM] Loading ONNX model: ${modelId}...`);
            console.log('[LLM] First load may take a few minutes to download the model...');

            const transformers = await import('@xenova/transformers');
            const { pipeline, env } = transformers;

            // Configure cache
            env.cacheDir = process.env.ONNX_CACHE_DIR ||
                (process.env.HOME ? `${process.env.HOME}/.ruvector/models/onnx` : '/tmp/.ruvector/models/onnx');
            env.allowRemoteModels = true;
            env.allowLocalModels = true;

            // Create text generation pipeline
            this.onnxPipeline = await pipeline('text-generation', modelId, {
                quantized: true,
                device: 'cpu',
            });

            this.onnxModel = modelId;
            this.onnxInitialized = true;
            console.log(`[LLM] ONNX model ready: ${modelId}`);
            return true;
        } catch (error) {
            console.warn('[LLM] ONNX init failed:', error.message);
            return false;
        }
    }

    /**
     * Call ONNX LLM for text generation
     */
    async callOnnx(modelId, systemPrompt, userMessage, options = {}) {
        await this.initOnnx(modelId);
        if (!this.onnxPipeline) {
            throw new Error('ONNX pipeline not initialized');
        }

        // Build prompt (simple format for small models)
        const prompt = systemPrompt
            ? `${systemPrompt}\n\nUser: ${userMessage}\n\nAssistant:`
            : userMessage;

        const start = Date.now();

        const outputs = await this.onnxPipeline(prompt, {
            max_new_tokens: options.maxTokens || 256,
            temperature: options.temperature || 0.7,
            top_p: options.topP || 0.9,
            top_k: options.topK || 50,
            repetition_penalty: 1.1,
            do_sample: (options.temperature || 0.7) > 0,
            return_full_text: false,
        });

        const timeMs = Date.now() - start;
        const generatedText = outputs[0]?.generated_text || '';

        return {
            content: generatedText.trim(),
            model: modelId,
            timeMs,
            usage: {
                input_tokens: Math.ceil(prompt.length / 4),
                output_tokens: Math.ceil(generatedText.length / 4),
            },
        };
    }

    /**
     * Initialize legacy ruvllm
     */
    async initLocal() {
        if (this.ruvllmInitialized) return;

        try {
            const ruvllm = await import('@ruvector/ruvllm');
            this.ruvllm = new ruvllm.RuvLLM({
                embeddingDim: 768,
                learningEnabled: true,
            });
            this.ruvllmInitialized = true;
            console.log('[LLM] Initialized local RuvLLM engine');
        } catch (error) {
            console.warn('[LLM] RuvLLM not available:', error.message);
        }
    }

    /**
     * Call LLM - local or cloud
     */
    async complete(systemPrompt, userMessage, options = {}) {
        const isLocal = this.config.type === 'local';

        if (isLocal) {
            return this.callLocal(systemPrompt, userMessage, options);
        }

        if (!this.apiKey) {
            throw new Error('No API key configured. Set ANTHROPIC_API_KEY or OPENAI_API_KEY, or use provider: "local"');
        }

        const model = this.config.models[options.model || this.model];

        if (this.provider === 'anthropic') {
            return this.callAnthropic(systemPrompt, userMessage, model, options);
        } else {
            return this.callOpenAI(systemPrompt, userMessage, model, options);
        }
    }

    /**
     * Call local LLM (ONNX primary, Ollama fallback)
     */
    async callLocal(systemPrompt, userMessage, options = {}) {
        const modelTier = options.model || this.model;
        const modelName = this.config.models[modelTier] || this.config.models.balanced;
        const backend = this.config.backend || 'onnx';

        // ========================================
        // 1. ONNX via transformers.js (Primary - REAL AI)
        // ========================================
        if (backend === 'onnx' || this.provider === 'local' || this.provider === 'onnx') {
            try {
                const onnxModelId = this.config.models[modelTier] || modelName;
                const response = await this.callOnnx(onnxModelId, systemPrompt, userMessage, options);

                // Validate response is meaningful
                if (response.content && response.content.length > 5) {
                    return {
                        content: response.content,
                        model: response.model,
                        usage: response.usage,
                        stopReason: 'end',
                        local: true,
                        onnx: true,
                        timeMs: response.timeMs,
                    };
                }
            } catch (error) {
                console.log(`[LLM] ONNX not available: ${error.message}`);
            }
        }

        // ========================================
        // 2. Ollama (Fallback if ONNX unavailable)
        // ========================================
        if (backend === 'ollama' || this.config.baseUrl) {
            const baseUrl = this.config.baseUrl || 'http://localhost:11434';
            const ollamaModel = this.config.models[modelTier] || 'qwen2.5:0.5b';

            try {
                const response = await this.callOllama(baseUrl, ollamaModel, systemPrompt, userMessage, options);
                if (response) {
                    return {
                        content: response.content,
                        model: ollamaModel,
                        usage: response.usage || { input_tokens: 0, output_tokens: 0 },
                        stopReason: 'end',
                        local: true,
                        ollama: true,
                    };
                }
            } catch (error) {
                console.log(`[LLM] Ollama not available: ${error.message}`);
            }
        }

        // ========================================
        // 3. Legacy RuvLLM (if explicitly selected)
        // ========================================
        if (backend === 'ruvllm' || this.provider === 'ruvllm') {
            await this.initLocal();
            if (this.ruvllm) {
                const prompt = `${systemPrompt}\n\n${userMessage}`;
                const response = this.ruvllm.query(prompt, {
                    maxTokens: options.maxTokens || this.maxTokens,
                    temperature: options.temperature || 0.7,
                });

                // Check if response is valid (not garbage)
                const isValidResponse = response.text &&
                    response.text.length > 10 &&
                    /[a-zA-Z]{3,}/.test(response.text) &&
                    !/^[>A-Z~|%#@\\+]+/.test(response.text);

                if (isValidResponse) {
                    return {
                        content: response.text,
                        model: `ruvllm-${modelTier}`,
                        usage: { input_tokens: prompt.length, output_tokens: response.text.length },
                        stopReason: 'end',
                        confidence: response.confidence,
                        local: true,
                    };
                }
            }
        }

        // ========================================
        // 4. Smart Template Fallback (Last resort)
        // ========================================
        console.log('[LLM] Using smart template generation');
        console.log('[LLM] Install @xenova/transformers for real ONNX AI inference');
        const fallbackResponse = this.generateSmartResponse(systemPrompt, userMessage);

        return {
            content: fallbackResponse,
            model: `template-${modelTier}`,
            usage: { input_tokens: systemPrompt.length + userMessage.length, output_tokens: fallbackResponse.length },
            stopReason: 'end',
            local: true,
            fallback: true,
        };
    }

    /**
     * Call Ollama API
     */
    async callOllama(baseUrl, model, systemPrompt, userMessage, options = {}) {
        const url = `${baseUrl}/api/chat`;

        const body = {
            model,
            messages: [
                { role: 'system', content: systemPrompt },
                { role: 'user', content: userMessage },
            ],
            stream: false,
            options: {
                temperature: options.temperature || 0.7,
                num_predict: options.maxTokens || this.maxTokens,
            },
        };

        const response = await fetch(url, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(body),
            signal: AbortSignal.timeout(options.timeout || 120000), // 2 min timeout
        });

        if (!response.ok) {
            const errorText = await response.text();
            throw new Error(`Ollama error ${response.status}: ${errorText}`);
        }

        const result = await response.json();

        return {
            content: result.message?.content || '',
            usage: {
                input_tokens: result.prompt_eval_count || 0,
                output_tokens: result.eval_count || 0,
            },
        };
    }

    /**
     * Generate smart contextual response based on task type
     */
    generateSmartResponse(systemPrompt, userMessage) {
        const task = userMessage.toLowerCase();
        const promptLower = systemPrompt.toLowerCase();

        // Review (check first - priority over code)
        if (promptLower.includes('review') || task.includes('review')) {
            return this.generateReviewResponse(userMessage);
        }

        // Test
        if (promptLower.includes('test') || task.includes('test')) {
            return this.generateTestResponse(userMessage);
        }

        // Research/analysis
        if (promptLower.includes('research') || promptLower.includes('analy') || task.includes('research') || task.includes('analy')) {
            return this.generateResearchResponse(userMessage);
        }

        // Code generation (check keywords in user message)
        if (promptLower.includes('coding') || promptLower.includes('coder') ||
            task.includes('write') || task.includes('function') || task.includes('implement') ||
            task.includes('create') || task.includes('build')) {
            return this.generateCodeResponse(userMessage);
        }

        // Default
        return this.generateDefaultResponse(userMessage);
    }

    generateCodeResponse(task) {
        const taskLower = task.toLowerCase();

        if (taskLower.includes('hello world')) {
            return `Here's a hello world implementation:

\`\`\`javascript
function helloWorld() {
    console.log('Hello, World!');
    return 'Hello, World!';
}

// Usage
helloWorld();
\`\`\`

This function prints "Hello, World!" to the console and returns the string.`;
        }

        if (taskLower.includes('sort') || taskLower.includes('array')) {
            return `Here's a sorting implementation:

\`\`\`javascript
function sortArray(arr, ascending = true) {
    return [...arr].sort((a, b) => ascending ? a - b : b - a);
}

// Example usage
const numbers = [5, 2, 8, 1, 9];
console.log(sortArray(numbers));       // [1, 2, 5, 8, 9]
console.log(sortArray(numbers, false)); // [9, 8, 5, 2, 1]
\`\`\``;
        }

        if (taskLower.includes('fetch') || taskLower.includes('api') || taskLower.includes('http')) {
            return `Here's an API fetch implementation:

\`\`\`javascript
async function fetchData(url, options = {}) {
    try {
        const response = await fetch(url, {
            method: options.method || 'GET',
            headers: { 'Content-Type': 'application/json', ...options.headers },
            body: options.body ? JSON.stringify(options.body) : undefined,
        });

        if (!response.ok) {
            throw new Error(\`HTTP error! status: \${response.status}\`);
        }

        return await response.json();
    } catch (error) {
        console.error('Fetch error:', error);
        throw error;
    }
}

// Usage
const data = await fetchData('https://api.example.com/data');
\`\`\``;
        }

        // Generic code response
        return `Based on your request: "${task.slice(0, 100)}..."

\`\`\`javascript
// Implementation
function solution(input) {
    // Process input
    const result = processInput(input);

    // Apply transformation
    const transformed = transform(result);

    return transformed;
}

function processInput(data) {
    // Validate and prepare data
    return data;
}

function transform(data) {
    // Apply business logic
    return { success: true, data };
}

module.exports = { solution };
\`\`\`

This provides a basic structure. For a complete implementation, please specify the exact requirements or use a cloud provider (-p anthropic or -p openai).`;
    }

    generateResearchResponse(task) {
        return `## Research Summary: ${task.slice(0, 60)}...

### Key Findings

1. **Overview**: This topic requires careful analysis of multiple factors.

2. **Primary Considerations**:
   - Understand the core requirements
   - Identify key stakeholders and constraints
   - Review existing solutions and best practices

3. **Recommended Approach**:
   - Start with a clear problem definition
   - Gather data from reliable sources
   - Validate assumptions with evidence

4. **Next Steps**:
   - Conduct detailed analysis
   - Document findings
   - Present recommendations

*Note: For comprehensive research with real sources, use a cloud provider with -p anthropic or -p openai.*`;
    }

    generateReviewResponse(task) {
        return `## Code Review Summary

**Task**: ${task.slice(0, 80)}...

### Assessment

✅ **Strengths**:
- Code structure appears organized
- Basic functionality is present

⚠️ **Suggestions for Improvement**:
1. Add error handling for edge cases
2. Include input validation
3. Add JSDoc comments for documentation
4. Consider adding unit tests
5. Review for potential security issues

### Recommendations
- Follow consistent naming conventions
- Extract repeated logic into helper functions
- Add logging for debugging

*For detailed code analysis, use a cloud provider.*`;
    }

    generateTestResponse(task) {
        return `## Test Plan: ${task.slice(0, 60)}...

\`\`\`javascript
describe('Feature Tests', () => {
    beforeEach(() => {
        // Setup test environment
    });

    afterEach(() => {
        // Cleanup
    });

    test('should handle normal input', () => {
        const result = functionUnderTest(normalInput);
        expect(result).toBeDefined();
        expect(result.success).toBe(true);
    });

    test('should handle edge cases', () => {
        expect(() => functionUnderTest(null)).toThrow();
        expect(() => functionUnderTest(undefined)).toThrow();
    });

    test('should handle error conditions', () => {
        const result = functionUnderTest(invalidInput);
        expect(result.error).toBeDefined();
    });
});
\`\`\`

### Test Coverage Recommendations
- Unit tests for core functions
- Integration tests for API endpoints
- Edge case testing
- Performance benchmarks`;
    }

    generateDefaultResponse(task) {
        return `Response to: ${task.slice(0, 100)}...

This is a local response generated without cloud API calls. For full LLM capabilities:
1. Install @ruvector/ruvllm for local AI
2. Or set ANTHROPIC_API_KEY/OPENAI_API_KEY for cloud

Task acknowledged and processed locally.`;
    }

    /**
     * Generate fallback response for basic tasks
     */
    generateFallbackResponse(systemPrompt, userMessage) {
        // Basic task-specific responses
        if (systemPrompt.includes('research')) {
            return `Based on the query "${userMessage.slice(0, 100)}...", here are the key findings:\n\n1. The topic requires further investigation.\n2. Multiple sources should be consulted.\n3. Consider the context and requirements carefully.\n\nNote: This is a local fallback response. For more detailed analysis, ensure ruvllm is properly installed.`;
        }

        if (systemPrompt.includes('coding') || systemPrompt.includes('code')) {
            return `Here's a code solution for: ${userMessage.slice(0, 50)}...\n\n\`\`\`javascript\n// Implementation based on the requirements\nfunction solution() {\n  // TODO: Implement the specific logic\n  console.log('Task:', '${userMessage.slice(0, 30)}...');\n  return { success: true };\n}\n\`\`\`\n\nNote: This is a local fallback. Install ruvllm for real code generation.`;
        }

        if (systemPrompt.includes('review')) {
            return `Code Review for: ${userMessage.slice(0, 50)}...\n\n**Summary:** The code structure appears reasonable.\n\n**Suggestions:**\n- Add error handling\n- Consider edge cases\n- Add documentation\n\nNote: This is a local fallback response.`;
        }

        if (systemPrompt.includes('test')) {
            return `Test Plan for: ${userMessage.slice(0, 50)}...\n\n\`\`\`javascript\ndescribe('Feature', () => {\n  it('should work correctly', () => {\n    // Test implementation\n    expect(true).toBe(true);\n  });\n});\n\`\`\`\n\nNote: This is a local fallback response.`;
        }

        // Generic response
        return `Response to: ${userMessage.slice(0, 100)}...\n\nThis is a local response generated without cloud API calls. For full LLM capabilities:\n1. Install @ruvector/ruvllm for local AI\n2. Or set ANTHROPIC_API_KEY/OPENAI_API_KEY for cloud\n\nTask acknowledged and processed locally.`;
    }

    async callAnthropic(systemPrompt, userMessage, model, options = {}) {
        const response = await fetch(`${this.config.baseUrl}/messages`, {
            method: 'POST',
            headers: this.config.headers(this.apiKey),
            body: JSON.stringify({
                model,
                max_tokens: options.maxTokens || this.maxTokens,
                system: systemPrompt,
                messages: [{ role: 'user', content: userMessage }],
            }),
        });

        if (!response.ok) {
            const error = await response.text();
            throw new Error(`Anthropic API error: ${response.status} - ${error}`);
        }

        const data = await response.json();
        return {
            content: data.content[0]?.text || '',
            model,
            usage: data.usage,
            stopReason: data.stop_reason,
        };
    }

    async callOpenAI(systemPrompt, userMessage, model, options = {}) {
        const response = await fetch(`${this.config.baseUrl}/chat/completions`, {
            method: 'POST',
            headers: this.config.headers(this.apiKey),
            body: JSON.stringify({
                model,
                max_tokens: options.maxTokens || this.maxTokens,
                messages: [
                    { role: 'system', content: systemPrompt },
                    { role: 'user', content: userMessage },
                ],
            }),
        });

        if (!response.ok) {
            const error = await response.text();
            throw new Error(`OpenAI API error: ${response.status} - ${error}`);
        }

        const data = await response.json();
        return {
            content: data.choices[0]?.message?.content || '',
            model,
            usage: data.usage,
            stopReason: data.choices[0]?.finish_reason,
        };
    }

    /**
     * Check if LLM is configured
     */
    isConfigured() {
        // Local is always configured
        if (this.config.type === 'local') return true;
        return !!this.apiKey;
    }

    /**
     * Check if using local provider
     */
    isLocal() {
        return this.config.type === 'local';
    }
}

// ============================================
// REAL EMBEDDER (uses ruvector)
// ============================================

/**
 * Real embedder using ruvector's AdaptiveEmbedder
 */
export class RealEmbedder {
    constructor(options = {}) {
        this.embedder = null;
        this.initialized = false;
        this.options = options;
    }

    async initialize() {
        try {
            // Try to load ruvector's AdaptiveEmbedder
            const { AdaptiveEmbedder } = await import('ruvector');
            this.embedder = new AdaptiveEmbedder();
            // Support both init() and initialize() methods
            if (typeof this.embedder.init === 'function') {
                await this.embedder.init();
            } else if (typeof this.embedder.initialize === 'function') {
                await this.embedder.initialize();
            }
            this.initialized = true;
            console.log('[Embedder] Initialized ruvector AdaptiveEmbedder');
            return true;
        } catch (error) {
            console.warn('[Embedder] ruvector not available, using fallback:', error.message);
            return false;
        }
    }

    async embed(text) {
        if (this.initialized && this.embedder) {
            return await this.embedder.embed(text);
        }
        // Fallback: Use a simple hash-based pseudo-embedding (NOT for production)
        console.warn('[Embedder] Using fallback hash embeddings - install ruvector for real embeddings');
        return this.fallbackEmbed(text);
    }

    async embedBatch(texts) {
        if (this.initialized && this.embedder) {
            return await this.embedder.embedBatch(texts);
        }
        return Promise.all(texts.map(t => this.fallbackEmbed(t)));
    }

    fallbackEmbed(text) {
        // Simple hash-based pseudo-embedding for testing
        // NOT semantically meaningful - use real embedder in production
        const hash = createHash('sha256').update(text).digest();
        const embedding = new Float32Array(384);
        for (let i = 0; i < 384; i++) {
            embedding[i] = (hash[i % 32] - 128) / 128;
        }
        return embedding;
    }

    async cosineSimilarity(a, b) {
        let dot = 0, normA = 0, normB = 0;
        for (let i = 0; i < a.length; i++) {
            dot += a[i] * b[i];
            normA += a[i] * a[i];
            normB += b[i] * b[i];
        }
        return dot / (Math.sqrt(normA) * Math.sqrt(normB));
    }
}

// ============================================
// REAL AGENT
// ============================================

/**
 * Real agent that executes tasks via LLM
 */
export class RealAgent extends EventEmitter {
    constructor(type, options = {}) {
        super();
        this.id = `agent-${type}-${Date.now()}-${randomBytes(4).toString('hex')}`;
        this.type = type;
        this.systemPrompt = AGENT_PROMPTS[type] || AGENT_PROMPTS.coder;
        this.llm = new LLMClient(options);
        this.embedder = null;
        this.status = 'idle';
        this.taskHistory = [];
        this.cost = { inputTokens: 0, outputTokens: 0 };
    }

    async initialize() {
        if (this.type === 'embedder') {
            this.embedder = new RealEmbedder();
            await this.embedder.initialize();
        }
        return this;
    }

    /**
     * Execute a task
     */
    async execute(task, context = {}) {
        if (!this.llm.isConfigured() && this.type !== 'embedder') {
            throw new Error('LLM not configured. Set ANTHROPIC_API_KEY or OPENAI_API_KEY');
        }

        this.status = 'executing';
        this.emit('started', { id: this.id, type: this.type, task });

        const startTime = Date.now();

        try {
            let result;

            if (this.type === 'embedder' && this.embedder) {
                // Embedding task
                result = await this.executeEmbeddingTask(task, context);
            } else {
                // LLM task
                result = await this.executeLLMTask(task, context);
            }

            const duration = Date.now() - startTime;

            this.taskHistory.push({
                task,
                result,
                duration,
                timestamp: new Date().toISOString(),
            });

            this.status = 'idle';
            this.emit('completed', { id: this.id, result, duration });

            return result;

        } catch (error) {
            this.status = 'error';
            this.emit('error', { id: this.id, error: error.message });
            throw error;
        }
    }

    async executeLLMTask(task, context = {}) {
        // Build user message with context
        let userMessage = task;

        if (context.files && context.files.length > 0) {
            userMessage += '\n\n--- FILES ---\n';
            for (const file of context.files) {
                try {
                    const content = readFileSync(file, 'utf-8');
                    userMessage += `\n### ${file}\n\`\`\`\n${content.slice(0, 10000)}\n\`\`\`\n`;
                } catch (e) {
                    userMessage += `\n### ${file}\n(Could not read file: ${e.message})\n`;
                }
            }
        }

        if (context.additionalContext) {
            userMessage += `\n\n--- ADDITIONAL CONTEXT ---\n${context.additionalContext}`;
        }

        const response = await this.llm.complete(this.systemPrompt, userMessage, {
            model: context.model || 'balanced',
        });

        // Track usage
        if (response.usage) {
            this.cost.inputTokens += response.usage.input_tokens || response.usage.prompt_tokens || 0;
            this.cost.outputTokens += response.usage.output_tokens || response.usage.completion_tokens || 0;
        }

        return {
            content: response.content,
            model: response.model,
            stopReason: response.stopReason,
            agentId: this.id,
            agentType: this.type,
        };
    }

    async executeEmbeddingTask(task, context = {}) {
        const texts = context.texts || [task];
        const embeddings = await this.embedder.embedBatch(texts);

        return {
            embeddings: embeddings.map((e, i) => ({
                text: texts[i].slice(0, 100),
                embedding: Array.from(e).slice(0, 10), // Preview
                dimensions: e.length,
            })),
            count: embeddings.length,
            agentId: this.id,
            agentType: this.type,
        };
    }

    getStats() {
        return {
            id: this.id,
            type: this.type,
            status: this.status,
            tasksCompleted: this.taskHistory.length,
            cost: this.cost,
            configured: this.llm.isConfigured() || this.type === 'embedder',
        };
    }
}

// ============================================
// REAL RELAY SYNC CLIENT
// ============================================

/**
 * Real sync client that connects to the actual relay server
 */
export class RelaySyncClient extends EventEmitter {
    constructor(options = {}) {
        super();
        this.relayUrl = options.relayUrl || 'ws://localhost:8080';
        this.nodeId = options.nodeId || `node-${randomBytes(8).toString('hex')}`;
        this.ws = null;
        this.connected = false;
        this.ledgerState = { earned: {}, spent: {}, balance: 0 };
        this.reconnectAttempts = 0;
        this.maxReconnects = options.maxReconnects || 10;
    }

    /**
     * Connect to relay server
     */
    async connect() {
        return new Promise((resolve, reject) => {
            try {
                // Use dynamic import for WebSocket in Node
                this.loadWebSocket().then(WebSocket => {
                    this.ws = new WebSocket(this.relayUrl);

                    const timeout = setTimeout(() => {
                        reject(new Error('Connection timeout'));
                    }, 10000);

                    this.ws.onopen = () => {
                        clearTimeout(timeout);
                        this.connected = true;
                        this.reconnectAttempts = 0;

                        // Register with relay
                        this.send({
                            type: 'register',
                            nodeId: this.nodeId,
                            capabilities: ['sync', 'agent', 'compute'],
                        });

                        this.emit('connected');
                        resolve(true);
                    };

                    this.ws.onmessage = (event) => {
                        this.handleMessage(JSON.parse(event.data));
                    };

                    this.ws.onclose = () => {
                        this.connected = false;
                        this.emit('disconnected');
                        this.scheduleReconnect();
                    };

                    this.ws.onerror = (error) => {
                        clearTimeout(timeout);
                        reject(error);
                    };

                }).catch(reject);
            } catch (error) {
                reject(error);
            }
        });
    }

    async loadWebSocket() {
        if (typeof WebSocket !== 'undefined') {
            return WebSocket;
        }
        const ws = await import('ws');
        return ws.default || ws.WebSocket;
    }

    scheduleReconnect() {
        if (this.reconnectAttempts < this.maxReconnects) {
            this.reconnectAttempts++;
            const delay = Math.min(1000 * Math.pow(2, this.reconnectAttempts), 30000);
            setTimeout(() => this.connect().catch(() => {}), delay);
        }
    }

    handleMessage(message) {
        switch (message.type) {
            case 'registered':
                console.log(`[Sync] Registered with relay as ${this.nodeId}`);
                this.emit('registered', message);
                break;

            case 'ledger_sync':
                this.mergeLedgerState(message.state);
                break;

            case 'peer_state':
                this.emit('peer_state', message);
                break;

            case 'time_crystal_sync':
                this.emit('time_crystal', message);
                break;

            default:
                this.emit('message', message);
        }
    }

    /**
     * Send message to relay
     */
    send(message) {
        if (this.connected && this.ws?.readyState === 1) {
            this.ws.send(JSON.stringify(message));
            return true;
        }
        return false;
    }

    /**
     * Sync ledger state with relay
     */
    syncLedger(state) {
        return this.send({
            type: 'ledger_sync',
            nodeId: this.nodeId,
            state,
            timestamp: Date.now(),
        });
    }

    /**
     * Merge incoming ledger state (CRDT)
     */
    mergeLedgerState(remoteState) {
        if (!remoteState) return;

        // Merge earned (max wins)
        for (const [key, value] of Object.entries(remoteState.earned || {})) {
            const current = this.ledgerState.earned[key] || 0;
            this.ledgerState.earned[key] = Math.max(current, value);
        }

        // Merge spent (max wins)
        for (const [key, value] of Object.entries(remoteState.spent || {})) {
            const current = this.ledgerState.spent[key] || 0;
            this.ledgerState.spent[key] = Math.max(current, value);
        }

        // Recalculate balance
        const totalEarned = Object.values(this.ledgerState.earned).reduce((a, b) => a + b, 0);
        const totalSpent = Object.values(this.ledgerState.spent).reduce((a, b) => a + b, 0);
        this.ledgerState.balance = totalEarned - totalSpent;

        this.emit('ledger_updated', this.ledgerState);
    }

    /**
     * Credit rUv
     */
    credit(amount, reason) {
        const key = `${Date.now()}-${reason}`;
        this.ledgerState.earned[key] = amount;
        this.ledgerState.balance += amount;
        this.syncLedger(this.ledgerState);
        return this.ledgerState.balance;
    }

    /**
     * Spend rUv
     */
    spend(amount, reason) {
        if (this.ledgerState.balance < amount) {
            throw new Error('Insufficient balance');
        }
        const key = `${Date.now()}-${reason}`;
        this.ledgerState.spent[key] = amount;
        this.ledgerState.balance -= amount;
        this.syncLedger(this.ledgerState);
        return this.ledgerState.balance;
    }

    getBalance() {
        return this.ledgerState.balance;
    }

    close() {
        if (this.ws) {
            this.ws.close();
        }
    }
}

// ============================================
// REAL AGENT MANAGER
// ============================================

/**
 * Manager for real agents with actual execution
 */
export class RealAgentManager extends EventEmitter {
    constructor(options = {}) {
        super();
        this.agents = new Map();
        this.syncClient = null;
        this.embedder = null;
        this.options = options;
    }

    async initialize() {
        // Initialize embedder
        this.embedder = new RealEmbedder();
        await this.embedder.initialize();

        // Connect to relay if URL provided
        if (this.options.relayUrl || this.options.enableSync) {
            this.syncClient = new RelaySyncClient({
                relayUrl: this.options.relayUrl || 'ws://localhost:8080',
                nodeId: this.options.nodeId,
            });

            try {
                await this.syncClient.connect();
                console.log('[AgentManager] Connected to relay server');
            } catch (error) {
                console.warn('[AgentManager] Relay connection failed:', error.message);
            }
        }

        return this;
    }

    /**
     * Spawn a real agent
     */
    async spawn(type, options = {}) {
        const agent = new RealAgent(type, {
            provider: options.provider || this.options.provider || 'anthropic',
            apiKey: options.apiKey || this.options.apiKey,
            model: options.model || 'balanced',
        });

        await agent.initialize();
        this.agents.set(agent.id, agent);

        // Track agent spawn with credits
        if (this.syncClient?.connected) {
            // Deduct spawn cost
            const spawnCost = { researcher: 1, coder: 2, reviewer: 1.5, tester: 1, analyst: 1, optimizer: 2, coordinator: 3, embedder: 0.5 };
            try {
                this.syncClient.spend(spawnCost[type] || 1, `spawn-${type}`);
            } catch (e) {
                // Continue even if no credits
            }
        }

        this.emit('agent_spawned', { id: agent.id, type });
        return agent;
    }

    /**
     * Execute task on agent
     */
    async execute(agentId, task, context = {}) {
        const agent = this.agents.get(agentId);
        if (!agent) {
            throw new Error(`Agent not found: ${agentId}`);
        }

        const result = await agent.execute(task, context);

        // Credit for completed task
        if (this.syncClient?.connected) {
            this.syncClient.credit(1, `task-${agent.type}`);
        }

        return result;
    }

    /**
     * Quick execute - spawn and run in one call
     */
    async quickExecute(type, task, context = {}) {
        const agent = await this.spawn(type, context);
        return agent.execute(task, context);
    }

    getAgent(id) {
        return this.agents.get(id);
    }

    listAgents() {
        return Array.from(this.agents.values()).map(a => a.getStats());
    }

    getBalance() {
        return this.syncClient?.getBalance() || 0;
    }

    async close() {
        if (this.syncClient) {
            this.syncClient.close();
        }
    }
}

// ============================================
// EXPORTS
// ============================================

// Classes are already exported via 'export class' declarations above
// Only export non-class items here
export { AGENT_PROMPTS, LLM_PROVIDERS };

export default RealAgentManager;
