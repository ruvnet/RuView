"use strict";
/**
 * RuvLLM Engine - Main orchestrator for self-learning LLM
 */
Object.defineProperty(exports, "__esModule", { value: true });
exports.RuvLLM = void 0;
const native_1 = require("./native");
/**
 * Convert JS config to native config format
 */
function toNativeConfig(config) {
    if (!config)
        return undefined;
    return {
        embedding_dim: config.embeddingDim,
        router_hidden_dim: config.routerHiddenDim,
        hnsw_m: config.hnswM,
        hnsw_ef_construction: config.hnswEfConstruction,
        hnsw_ef_search: config.hnswEfSearch,
        learning_enabled: config.learningEnabled,
        quality_threshold: config.qualityThreshold,
        ewc_lambda: config.ewcLambda,
    };
}
/**
 * Convert JS generation config to native format
 */
function toNativeGenConfig(config) {
    if (!config)
        return undefined;
    return {
        max_tokens: config.maxTokens,
        temperature: config.temperature,
        top_p: config.topP,
        top_k: config.topK,
        repetition_penalty: config.repetitionPenalty,
    };
}
/**
 * RuvLLM - Self-learning LLM orchestrator
 *
 * Combines SONA adaptive learning with HNSW memory,
 * FastGRNN routing, and SIMD-optimized inference.
 *
 * @example
 * ```typescript
 * import { RuvLLM } from '@ruvector/ruvllm';
 *
 * const llm = new RuvLLM({ embeddingDim: 768 });
 *
 * // Query with automatic routing
 * const response = await llm.query('What is machine learning?');
 * console.log(response.text);
 *
 * // Provide feedback for learning
 * llm.feedback({ requestId: response.requestId, rating: 5 });
 * ```
 */
class RuvLLM {
    /**
     * Create a new RuvLLM instance
     */
    constructor(config) {
        this.native = null;
        // Fallback state for when native module is not available
        this.fallbackState = {
            memory: new Map(),
            nextId: 1,
            queryCount: 0,
        };
        this.config = config ?? {};
        const mod = (0, native_1.getNativeModule)();
        if (mod) {
            try {
                this.native = new mod.RuvLLMEngine(toNativeConfig(config));
            }
            catch {
                // Silently fall back to JS implementation
            }
        }
    }
    /**
     * Query the LLM with automatic routing
     */
    query(text, config) {
        if (this.native) {
            const result = this.native.query(text, toNativeGenConfig(config));
            return {
                text: result.text,
                confidence: result.confidence,
                model: result.model,
                contextSize: result.context_size,
                latencyMs: result.latency_ms,
                requestId: result.request_id,
            };
        }
        // Fallback implementation
        this.fallbackState.queryCount++;
        return {
            text: `[Fallback] Response to: ${text.slice(0, 50)}...`,
            confidence: 0.5,
            model: 'fallback',
            contextSize: 512,
            latencyMs: 1.0,
            requestId: `fb-${Date.now()}-${Math.random().toString(36).slice(2)}`,
        };
    }
    /**
     * Generate text with SIMD-optimized inference
     *
     * Note: If no trained model is loaded (demo mode), returns an informational
     * message instead of garbled output.
     */
    generate(prompt, config) {
        if (this.native) {
            return this.native.generate(prompt, toNativeGenConfig(config));
        }
        // Fallback - provide helpful message instead of garbled output
        const maxTokens = config?.maxTokens ?? 256;
        const temp = config?.temperature ?? 0.7;
        const topP = config?.topP ?? 0.9;
        return `[RuvLLM JavaScript Fallback Mode]
No native SIMD module loaded. Running in JavaScript fallback mode.

Your prompt: "${prompt.slice(0, 100)}${prompt.length > 100 ? '...' : ''}"

To enable native SIMD inference:
1. Install the native bindings: npm install @ruvector/ruvllm-${process.platform}-${process.arch}
2. Or load a GGUF model file
3. Or connect to an external LLM API

Config: temp=${temp.toFixed(2)}, top_p=${topP.toFixed(2)}, max_tokens=${maxTokens}

This fallback provides routing, memory, and embedding features but not full text generation.`;
    }
    /**
     * Get routing decision for a query
     */
    route(text) {
        if (this.native) {
            const result = this.native.route(text);
            return {
                model: result.model,
                contextSize: result.context_size,
                temperature: result.temperature,
                topP: result.top_p,
                confidence: result.confidence,
            };
        }
        // Fallback
        return {
            model: 'M700',
            contextSize: 512,
            temperature: 0.7,
            topP: 0.9,
            confidence: 0.5,
        };
    }
    /**
     * Search memory for similar content
     */
    searchMemory(text, k = 10) {
        if (this.native) {
            const results = this.native.searchMemory(text, k);
            return results.map(r => ({
                id: r.id,
                score: r.score,
                content: r.content,
                metadata: JSON.parse(r.metadata || '{}'),
            }));
        }
        // Fallback - simple search
        return Array.from(this.fallbackState.memory.entries())
            .slice(0, k)
            .map(([id, data]) => ({
            id,
            score: 0.5,
            content: data.content,
            metadata: data.metadata,
        }));
    }
    /**
     * Add content to memory
     */
    addMemory(content, metadata) {
        if (this.native) {
            return this.native.addMemory(content, metadata ? JSON.stringify(metadata) : undefined);
        }
        // Fallback
        const id = this.fallbackState.nextId++;
        this.fallbackState.memory.set(id, {
            content,
            embedding: this.embed(content),
            metadata: metadata ?? {},
        });
        return id;
    }
    /**
     * Provide feedback for learning
     */
    feedback(fb) {
        if (this.native) {
            return this.native.feedback(fb.requestId, fb.rating, fb.correction);
        }
        return false;
    }
    /**
     * Get engine statistics
     */
    stats() {
        if (this.native) {
            const s = this.native.stats();
            // Map native stats (snake_case) to TypeScript interface (camelCase)
            // Handle both old and new field names for backward compatibility
            return {
                totalQueries: s.total_queries ?? 0,
                memoryNodes: s.memory_nodes ?? 0,
                patternsLearned: s.patterns_learned ?? s.training_steps ?? 0,
                avgLatencyMs: s.avg_latency_ms ?? 0,
                cacheHitRate: s.cache_hit_rate ?? 0,
                routerAccuracy: s.router_accuracy ?? 0.5,
            };
        }
        // Fallback
        return {
            totalQueries: this.fallbackState.queryCount,
            memoryNodes: this.fallbackState.memory.size,
            patternsLearned: 0,
            avgLatencyMs: 1.0,
            cacheHitRate: 0.0,
            routerAccuracy: 0.5,
        };
    }
    /**
     * Force router learning cycle
     */
    forceLearn() {
        if (this.native) {
            return this.native.forceLearn();
        }
        return 'Learning not available in fallback mode';
    }
    /**
     * Get embedding for text
     */
    embed(text) {
        if (this.native) {
            return this.native.embed(text);
        }
        // Fallback - simple hash-based embedding
        const dim = this.config.embeddingDim ?? 768;
        const embedding = new Array(dim).fill(0);
        for (let i = 0; i < text.length; i++) {
            const idx = (text.charCodeAt(i) * (i + 1)) % dim;
            embedding[idx] += 0.1;
        }
        // Normalize
        const norm = Math.sqrt(embedding.reduce((sum, x) => sum + x * x, 0)) || 1;
        return embedding.map(x => x / norm);
    }
    /**
     * Compute similarity between two texts
     */
    similarity(text1, text2) {
        if (this.native) {
            return this.native.similarity(text1, text2);
        }
        // Fallback - cosine similarity
        const emb1 = this.embed(text1);
        const emb2 = this.embed(text2);
        let dot = 0;
        let norm1 = 0;
        let norm2 = 0;
        for (let i = 0; i < emb1.length; i++) {
            dot += emb1[i] * emb2[i];
            norm1 += emb1[i] * emb1[i];
            norm2 += emb2[i] * emb2[i];
        }
        const denom = Math.sqrt(norm1) * Math.sqrt(norm2);
        const similarity = denom > 0 ? dot / denom : 0;
        // Clamp to [0, 1] to handle floating point errors
        return Math.max(0, Math.min(1, similarity));
    }
    /**
     * Check if SIMD is available
     */
    hasSimd() {
        if (this.native) {
            return this.native.hasSimd();
        }
        return false;
    }
    /**
     * Get SIMD capabilities
     */
    simdCapabilities() {
        if (this.native) {
            return this.native.simdCapabilities();
        }
        return ['Scalar (fallback)'];
    }
    /**
     * Batch query multiple prompts
     */
    batchQuery(request) {
        const start = Date.now();
        const responses = request.queries.map(q => this.query(q, request.config));
        return {
            responses,
            totalLatencyMs: Date.now() - start,
        };
    }
    /**
     * Check if native module is loaded
     */
    isNativeLoaded() {
        return this.native !== null;
    }
}
exports.RuvLLM = RuvLLM;
//# sourceMappingURL=engine.js.map