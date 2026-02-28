"use strict";
/**
 * WASM Mock Module
 *
 * Mock implementations for RuVector WASM bindings
 * Used to test code that depends on WASM modules without loading actual binaries
 */
Object.defineProperty(exports, "__esModule", { value: true });
exports.mockWasmLoader = exports.MockWasmRouter = exports.MockWasmEmbedder = exports.MockWasmVectorIndex = void 0;
exports.createMockRuVectorBindings = createMockRuVectorBindings;
exports.resetWasmMocks = resetWasmMocks;
const vitest_1 = require("vitest");
// Mock implementations
/**
 * Mock WASM Vector Index
 */
class MockWasmVectorIndex {
    constructor(dimension = 384) {
        this.vectors = new Map();
        this.dimension = dimension;
    }
    add(id, vector) {
        if (vector.length !== this.dimension) {
            throw new Error(`Vector dimension mismatch: expected ${this.dimension}, got ${vector.length}`);
        }
        this.vectors.set(id, vector);
    }
    search(query, topK) {
        if (query.length !== this.dimension) {
            throw new Error(`Query dimension mismatch: expected ${this.dimension}, got ${query.length}`);
        }
        const results = [];
        for (const [id, vector] of this.vectors) {
            const distance = this.cosineSimilarity(query, vector);
            results.push({
                id,
                score: distance,
                distance: 1 - distance
            });
        }
        return results
            .sort((a, b) => b.score - a.score)
            .slice(0, topK);
    }
    delete(id) {
        return this.vectors.delete(id);
    }
    size() {
        return this.vectors.size;
    }
    clear() {
        this.vectors.clear();
    }
    cosineSimilarity(a, b) {
        let dotProduct = 0;
        let normA = 0;
        let normB = 0;
        for (let i = 0; i < a.length; i++) {
            dotProduct += a[i] * b[i];
            normA += a[i] * a[i];
            normB += b[i] * b[i];
        }
        return dotProduct / (Math.sqrt(normA) * Math.sqrt(normB));
    }
}
exports.MockWasmVectorIndex = MockWasmVectorIndex;
/**
 * Mock WASM Embedder
 */
class MockWasmEmbedder {
    constructor(dimension = 384) {
        this.cache = new Map();
        this.dim = dimension;
    }
    embed(text) {
        // Check cache first
        if (this.cache.has(text)) {
            return this.cache.get(text);
        }
        // Generate deterministic pseudo-random embedding based on text hash
        const embedding = new Float32Array(this.dim);
        let hash = this.hashCode(text);
        for (let i = 0; i < this.dim; i++) {
            hash = ((hash * 1103515245) + 12345) & 0x7fffffff;
            embedding[i] = (hash / 0x7fffffff) * 2 - 1;
        }
        // Normalize the embedding
        const norm = Math.sqrt(embedding.reduce((sum, val) => sum + val * val, 0));
        for (let i = 0; i < this.dim; i++) {
            embedding[i] /= norm;
        }
        this.cache.set(text, embedding);
        return embedding;
    }
    embedBatch(texts) {
        return texts.map(text => this.embed(text));
    }
    dimension() {
        return this.dim;
    }
    hashCode(str) {
        let hash = 0;
        for (let i = 0; i < str.length; i++) {
            const char = str.charCodeAt(i);
            hash = ((hash << 5) - hash) + char;
            hash = hash & hash;
        }
        return Math.abs(hash);
    }
}
exports.MockWasmEmbedder = MockWasmEmbedder;
/**
 * Mock WASM Router
 */
class MockWasmRouter {
    constructor() {
        this.routes = new Map();
    }
    route(input, context) {
        for (const [key, route] of this.routes) {
            if (route.pattern.test(input)) {
                return {
                    handler: route.handler,
                    confidence: 0.95,
                    metadata: { matchedPattern: key, context }
                };
            }
        }
        // Default fallback
        return {
            handler: 'default',
            confidence: 0.5,
            metadata: { fallback: true, context }
        };
    }
    addRoute(pattern, handler) {
        this.routes.set(pattern, {
            pattern: new RegExp(pattern, 'i'),
            handler
        });
    }
    removeRoute(pattern) {
        return this.routes.delete(pattern);
    }
}
exports.MockWasmRouter = MockWasmRouter;
/**
 * Mock WASM Module Loader
 */
exports.mockWasmLoader = {
    loadVectorIndex: vitest_1.vi.fn(async (dimension) => {
        return new MockWasmVectorIndex(dimension);
    }),
    loadEmbedder: vitest_1.vi.fn(async (dimension) => {
        return new MockWasmEmbedder(dimension);
    }),
    loadRouter: vitest_1.vi.fn(async () => {
        return new MockWasmRouter();
    }),
    isWasmSupported: vitest_1.vi.fn(() => true),
    getWasmMemory: vitest_1.vi.fn(() => ({
        used: 1024 * 1024 * 50, // 50MB
        total: 1024 * 1024 * 256 // 256MB
    }))
};
/**
 * Create mock WASM bindings for RuVector
 */
function createMockRuVectorBindings() {
    const vectorIndex = new MockWasmVectorIndex(384);
    const embedder = new MockWasmEmbedder(384);
    const router = new MockWasmRouter();
    return {
        vectorIndex,
        embedder,
        router,
        // Convenience methods
        async search(query, topK = 10) {
            const embedding = embedder.embed(query);
            return vectorIndex.search(embedding, topK);
        },
        async index(id, text) {
            const embedding = embedder.embed(text);
            vectorIndex.add(id, embedding);
        },
        async batchIndex(items) {
            for (const item of items) {
                const embedding = embedder.embed(item.text);
                vectorIndex.add(item.id, embedding);
            }
        }
    };
}
/**
 * Reset all WASM mocks
 */
function resetWasmMocks() {
    vitest_1.vi.clearAllMocks();
    exports.mockWasmLoader.loadVectorIndex.mockClear();
    exports.mockWasmLoader.loadEmbedder.mockClear();
    exports.mockWasmLoader.loadRouter.mockClear();
}
// Default export for easy mocking
exports.default = {
    MockWasmVectorIndex,
    MockWasmEmbedder,
    MockWasmRouter,
    mockWasmLoader: exports.mockWasmLoader,
    createMockRuVectorBindings,
    resetWasmMocks
};
//# sourceMappingURL=wasm.mock.js.map