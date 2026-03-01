/**
 * WASM Mock Module
 *
 * Mock implementations for RuVector WASM bindings
 * Used to test code that depends on WASM modules without loading actual binaries
 */

import { vi } from 'vitest';

// Types for WASM interfaces
export interface WasmVectorIndex {
  add(id: string, vector: Float32Array): void;
  search(query: Float32Array, topK: number): SearchResult[];
  delete(id: string): boolean;
  size(): number;
  clear(): void;
}

export interface SearchResult {
  id: string;
  score: number;
  distance: number;
}

export interface WasmEmbedder {
  embed(text: string): Float32Array;
  embedBatch(texts: string[]): Float32Array[];
  dimension(): number;
}

export interface WasmRouter {
  route(input: string, context?: Record<string, unknown>): RouteResult;
  addRoute(pattern: string, handler: string): void;
  removeRoute(pattern: string): boolean;
}

export interface RouteResult {
  handler: string;
  confidence: number;
  metadata: Record<string, unknown>;
}

// Mock implementations

/**
 * Mock WASM Vector Index
 */
export class MockWasmVectorIndex implements WasmVectorIndex {
  private vectors: Map<string, Float32Array> = new Map();
  private dimension: number;

  constructor(dimension: number = 384) {
    this.dimension = dimension;
  }

  add(id: string, vector: Float32Array): void {
    if (vector.length !== this.dimension) {
      throw new Error(`Vector dimension mismatch: expected ${this.dimension}, got ${vector.length}`);
    }
    this.vectors.set(id, vector);
  }

  search(query: Float32Array, topK: number): SearchResult[] {
    if (query.length !== this.dimension) {
      throw new Error(`Query dimension mismatch: expected ${this.dimension}, got ${query.length}`);
    }

    const results: SearchResult[] = [];

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

  delete(id: string): boolean {
    return this.vectors.delete(id);
  }

  size(): number {
    return this.vectors.size;
  }

  clear(): void {
    this.vectors.clear();
  }

  private cosineSimilarity(a: Float32Array, b: Float32Array): number {
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

/**
 * Mock WASM Embedder
 */
export class MockWasmEmbedder implements WasmEmbedder {
  private dim: number;
  private cache: Map<string, Float32Array> = new Map();

  constructor(dimension: number = 384) {
    this.dim = dimension;
  }

  embed(text: string): Float32Array {
    // Check cache first
    if (this.cache.has(text)) {
      return this.cache.get(text)!;
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

  embedBatch(texts: string[]): Float32Array[] {
    return texts.map(text => this.embed(text));
  }

  dimension(): number {
    return this.dim;
  }

  private hashCode(str: string): number {
    let hash = 0;
    for (let i = 0; i < str.length; i++) {
      const char = str.charCodeAt(i);
      hash = ((hash << 5) - hash) + char;
      hash = hash & hash;
    }
    return Math.abs(hash);
  }
}

/**
 * Mock WASM Router
 */
export class MockWasmRouter implements WasmRouter {
  private routes: Map<string, { pattern: RegExp; handler: string }> = new Map();

  route(input: string, context?: Record<string, unknown>): RouteResult {
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

  addRoute(pattern: string, handler: string): void {
    this.routes.set(pattern, {
      pattern: new RegExp(pattern, 'i'),
      handler
    });
  }

  removeRoute(pattern: string): boolean {
    return this.routes.delete(pattern);
  }
}

/**
 * Mock WASM Module Loader
 */
export const mockWasmLoader = {
  loadVectorIndex: vi.fn(async (dimension?: number): Promise<WasmVectorIndex> => {
    return new MockWasmVectorIndex(dimension);
  }),

  loadEmbedder: vi.fn(async (dimension?: number): Promise<WasmEmbedder> => {
    return new MockWasmEmbedder(dimension);
  }),

  loadRouter: vi.fn(async (): Promise<WasmRouter> => {
    return new MockWasmRouter();
  }),

  isWasmSupported: vi.fn((): boolean => true),

  getWasmMemory: vi.fn((): { used: number; total: number } => ({
    used: 1024 * 1024 * 50,  // 50MB
    total: 1024 * 1024 * 256 // 256MB
  }))
};

/**
 * Create mock WASM bindings for RuVector
 */
export function createMockRuVectorBindings() {
  const vectorIndex = new MockWasmVectorIndex(384);
  const embedder = new MockWasmEmbedder(384);
  const router = new MockWasmRouter();

  return {
    vectorIndex,
    embedder,
    router,

    // Convenience methods
    async search(query: string, topK: number = 10): Promise<SearchResult[]> {
      const embedding = embedder.embed(query);
      return vectorIndex.search(embedding, topK);
    },

    async index(id: string, text: string): Promise<void> {
      const embedding = embedder.embed(text);
      vectorIndex.add(id, embedding);
    },

    async batchIndex(items: Array<{ id: string; text: string }>): Promise<void> {
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
export function resetWasmMocks(): void {
  vi.clearAllMocks();
  mockWasmLoader.loadVectorIndex.mockClear();
  mockWasmLoader.loadEmbedder.mockClear();
  mockWasmLoader.loadRouter.mockClear();
}

// Default export for easy mocking
export default {
  MockWasmVectorIndex,
  MockWasmEmbedder,
  MockWasmRouter,
  mockWasmLoader,
  createMockRuVectorBindings,
  resetWasmMocks
};
