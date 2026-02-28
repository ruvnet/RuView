/**
 * Memory Domain Entity - Unit Tests
 *
 * Tests for Memory storage, retrieval, and vector operations
 */

import { describe, it, expect, beforeEach, vi } from 'vitest';
import { createMemory, createVectorMemory, type Memory, type MemoryMetadata } from '../../factories';

// Memory Entity Types
interface MemoryEntry {
  id: string;
  tenantId: string;
  sessionId: string | null;
  type: 'short-term' | 'long-term' | 'vector' | 'episodic';
  key: string;
  value: unknown;
  embedding: Float32Array | null;
  metadata: MemoryEntryMetadata;
}

interface MemoryEntryMetadata {
  createdAt: Date;
  updatedAt: Date;
  expiresAt: Date | null;
  accessCount: number;
  importance: number;
  tags: string[];
}

interface VectorSearchResult {
  entry: MemoryEntry;
  score: number;
  distance: number;
}

// Mock Memory Store class for testing
class MemoryStore {
  private entries: Map<string, MemoryEntry> = new Map();
  private indexByKey: Map<string, Set<string>> = new Map();
  private indexByTenant: Map<string, Set<string>> = new Map();
  private indexBySession: Map<string, Set<string>> = new Map();
  private readonly dimension: number;

  constructor(dimension: number = 384) {
    this.dimension = dimension;
  }

  async set(entry: Omit<MemoryEntry, 'metadata'> & { metadata?: Partial<MemoryEntryMetadata> }): Promise<MemoryEntry> {
    const fullEntry: MemoryEntry = {
      ...entry,
      metadata: {
        createdAt: entry.metadata?.createdAt || new Date(),
        updatedAt: new Date(),
        expiresAt: entry.metadata?.expiresAt || null,
        accessCount: entry.metadata?.accessCount || 0,
        importance: entry.metadata?.importance || 0.5,
        tags: entry.metadata?.tags || []
      }
    };

    // Validate embedding dimension
    if (fullEntry.embedding && fullEntry.embedding.length !== this.dimension) {
      throw new Error(`Embedding dimension mismatch: expected ${this.dimension}, got ${fullEntry.embedding.length}`);
    }

    this.entries.set(entry.id, fullEntry);
    this.updateIndexes(fullEntry);

    return fullEntry;
  }

  async get(id: string): Promise<MemoryEntry | null> {
    const entry = this.entries.get(id);
    if (entry) {
      entry.metadata.accessCount++;
      entry.metadata.updatedAt = new Date();
    }
    return entry || null;
  }

  async getByKey(key: string, tenantId: string): Promise<MemoryEntry | null> {
    const ids = this.indexByKey.get(key);
    if (!ids) return null;

    for (const id of ids) {
      const entry = this.entries.get(id);
      if (entry && entry.tenantId === tenantId) {
        entry.metadata.accessCount++;
        return entry;
      }
    }
    return null;
  }

  async delete(id: string): Promise<boolean> {
    const entry = this.entries.get(id);
    if (!entry) return false;

    this.removeFromIndexes(entry);
    return this.entries.delete(id);
  }

  async deleteByKey(key: string, tenantId: string): Promise<boolean> {
    const entry = await this.getByKey(key, tenantId);
    if (!entry) return false;
    return this.delete(entry.id);
  }

  async listByTenant(tenantId: string, limit: number = 100): Promise<MemoryEntry[]> {
    const ids = this.indexByTenant.get(tenantId);
    if (!ids) return [];

    const entries: MemoryEntry[] = [];
    for (const id of ids) {
      const entry = this.entries.get(id);
      if (entry) entries.push(entry);
      if (entries.length >= limit) break;
    }
    return entries;
  }

  async listBySession(sessionId: string, limit: number = 100): Promise<MemoryEntry[]> {
    const ids = this.indexBySession.get(sessionId);
    if (!ids) return [];

    const entries: MemoryEntry[] = [];
    for (const id of ids) {
      const entry = this.entries.get(id);
      if (entry) entries.push(entry);
      if (entries.length >= limit) break;
    }
    return entries;
  }

  async search(query: Float32Array, tenantId: string, topK: number = 10): Promise<VectorSearchResult[]> {
    if (query.length !== this.dimension) {
      throw new Error(`Query dimension mismatch: expected ${this.dimension}, got ${query.length}`);
    }

    const results: VectorSearchResult[] = [];
    const tenantIds = this.indexByTenant.get(tenantId);
    if (!tenantIds) return [];

    for (const id of tenantIds) {
      const entry = this.entries.get(id);
      if (entry?.embedding) {
        const score = this.cosineSimilarity(query, entry.embedding);
        results.push({
          entry,
          score,
          distance: 1 - score
        });
      }
    }

    return results
      .sort((a, b) => b.score - a.score)
      .slice(0, topK);
  }

  async expire(): Promise<number> {
    const now = new Date();
    let expiredCount = 0;

    for (const [id, entry] of this.entries) {
      if (entry.metadata.expiresAt && entry.metadata.expiresAt < now) {
        this.delete(id);
        expiredCount++;
      }
    }

    return expiredCount;
  }

  async clear(tenantId?: string): Promise<number> {
    if (tenantId) {
      const ids = this.indexByTenant.get(tenantId);
      if (!ids) return 0;

      let deletedCount = 0;
      for (const id of Array.from(ids)) {
        if (this.delete(id)) deletedCount++;
      }
      return deletedCount;
    }

    const count = this.entries.size;
    this.entries.clear();
    this.indexByKey.clear();
    this.indexByTenant.clear();
    this.indexBySession.clear();
    return count;
  }

  size(): number {
    return this.entries.size;
  }

  sizeByTenant(tenantId: string): number {
    return this.indexByTenant.get(tenantId)?.size || 0;
  }

  private updateIndexes(entry: MemoryEntry): void {
    // Key index
    let keySet = this.indexByKey.get(entry.key);
    if (!keySet) {
      keySet = new Set();
      this.indexByKey.set(entry.key, keySet);
    }
    keySet.add(entry.id);

    // Tenant index
    let tenantSet = this.indexByTenant.get(entry.tenantId);
    if (!tenantSet) {
      tenantSet = new Set();
      this.indexByTenant.set(entry.tenantId, tenantSet);
    }
    tenantSet.add(entry.id);

    // Session index
    if (entry.sessionId) {
      let sessionSet = this.indexBySession.get(entry.sessionId);
      if (!sessionSet) {
        sessionSet = new Set();
        this.indexBySession.set(entry.sessionId, sessionSet);
      }
      sessionSet.add(entry.id);
    }
  }

  private removeFromIndexes(entry: MemoryEntry): void {
    this.indexByKey.get(entry.key)?.delete(entry.id);
    this.indexByTenant.get(entry.tenantId)?.delete(entry.id);
    if (entry.sessionId) {
      this.indexBySession.get(entry.sessionId)?.delete(entry.id);
    }
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

    const denominator = Math.sqrt(normA) * Math.sqrt(normB);
    return denominator === 0 ? 0 : dotProduct / denominator;
  }
}

// Tests
describe('Memory Store', () => {
  let store: MemoryStore;

  beforeEach(() => {
    store = new MemoryStore(384);
  });

  describe('Basic Operations', () => {
    it('should set and get memory entry', async () => {
      const entry = await store.set({
        id: 'mem-001',
        tenantId: 'tenant-001',
        sessionId: null,
        type: 'long-term',
        key: 'test-key',
        value: { data: 'test' },
        embedding: null
      });

      const retrieved = await store.get('mem-001');

      expect(retrieved).not.toBeNull();
      expect(retrieved?.id).toBe('mem-001');
      expect(retrieved?.value).toEqual({ data: 'test' });
    });

    it('should return null for non-existent entry', async () => {
      const entry = await store.get('non-existent');
      expect(entry).toBeNull();
    });

    it('should increment access count on get', async () => {
      await store.set({
        id: 'mem-001',
        tenantId: 'tenant-001',
        sessionId: null,
        type: 'short-term',
        key: 'test',
        value: 'test',
        embedding: null
      });

      await store.get('mem-001');
      await store.get('mem-001');
      const entry = await store.get('mem-001');

      expect(entry?.metadata.accessCount).toBe(3);
    });

    it('should delete entry', async () => {
      await store.set({
        id: 'mem-001',
        tenantId: 'tenant-001',
        sessionId: null,
        type: 'short-term',
        key: 'test',
        value: 'test',
        embedding: null
      });

      const deleted = await store.delete('mem-001');
      const entry = await store.get('mem-001');

      expect(deleted).toBe(true);
      expect(entry).toBeNull();
    });

    it('should return false when deleting non-existent entry', async () => {
      const deleted = await store.delete('non-existent');
      expect(deleted).toBe(false);
    });
  });

  describe('Key-based Operations', () => {
    it('should get entry by key and tenant', async () => {
      await store.set({
        id: 'mem-001',
        tenantId: 'tenant-001',
        sessionId: null,
        type: 'long-term',
        key: 'unique-key',
        value: 'value1',
        embedding: null
      });

      await store.set({
        id: 'mem-002',
        tenantId: 'tenant-002',
        sessionId: null,
        type: 'long-term',
        key: 'unique-key',
        value: 'value2',
        embedding: null
      });

      const entry1 = await store.getByKey('unique-key', 'tenant-001');
      const entry2 = await store.getByKey('unique-key', 'tenant-002');

      expect(entry1?.value).toBe('value1');
      expect(entry2?.value).toBe('value2');
    });

    it('should delete by key', async () => {
      await store.set({
        id: 'mem-001',
        tenantId: 'tenant-001',
        sessionId: null,
        type: 'long-term',
        key: 'to-delete',
        value: 'test',
        embedding: null
      });

      const deleted = await store.deleteByKey('to-delete', 'tenant-001');
      const entry = await store.getByKey('to-delete', 'tenant-001');

      expect(deleted).toBe(true);
      expect(entry).toBeNull();
    });
  });

  describe('Listing Operations', () => {
    beforeEach(async () => {
      for (let i = 0; i < 5; i++) {
        await store.set({
          id: `mem-${i}`,
          tenantId: 'tenant-001',
          sessionId: 'session-001',
          type: 'short-term',
          key: `key-${i}`,
          value: `value-${i}`,
          embedding: null
        });
      }

      for (let i = 5; i < 8; i++) {
        await store.set({
          id: `mem-${i}`,
          tenantId: 'tenant-002',
          sessionId: 'session-002',
          type: 'short-term',
          key: `key-${i}`,
          value: `value-${i}`,
          embedding: null
        });
      }
    });

    it('should list entries by tenant', async () => {
      const entries = await store.listByTenant('tenant-001');
      expect(entries).toHaveLength(5);
      entries.forEach(e => expect(e.tenantId).toBe('tenant-001'));
    });

    it('should list entries by session', async () => {
      const entries = await store.listBySession('session-001');
      expect(entries).toHaveLength(5);
      entries.forEach(e => expect(e.sessionId).toBe('session-001'));
    });

    it('should respect limit parameter', async () => {
      const entries = await store.listByTenant('tenant-001', 3);
      expect(entries).toHaveLength(3);
    });

    it('should return empty array for unknown tenant', async () => {
      const entries = await store.listByTenant('unknown');
      expect(entries).toEqual([]);
    });
  });

  describe('Vector Operations', () => {
    const createRandomEmbedding = (dim: number): Float32Array => {
      const arr = new Float32Array(dim);
      let norm = 0;
      for (let i = 0; i < dim; i++) {
        arr[i] = Math.random() - 0.5;
        norm += arr[i] * arr[i];
      }
      norm = Math.sqrt(norm);
      for (let i = 0; i < dim; i++) {
        arr[i] /= norm;
      }
      return arr;
    };

    it('should search by vector similarity', async () => {
      const embedding1 = createRandomEmbedding(384);
      const embedding2 = createRandomEmbedding(384);
      const embedding3 = createRandomEmbedding(384);

      await store.set({
        id: 'vec-1',
        tenantId: 'tenant-001',
        sessionId: null,
        type: 'vector',
        key: 'doc-1',
        value: { text: 'Document 1' },
        embedding: embedding1
      });

      await store.set({
        id: 'vec-2',
        tenantId: 'tenant-001',
        sessionId: null,
        type: 'vector',
        key: 'doc-2',
        value: { text: 'Document 2' },
        embedding: embedding2
      });

      await store.set({
        id: 'vec-3',
        tenantId: 'tenant-001',
        sessionId: null,
        type: 'vector',
        key: 'doc-3',
        value: { text: 'Document 3' },
        embedding: embedding3
      });

      const results = await store.search(embedding1, 'tenant-001', 2);

      expect(results).toHaveLength(2);
      expect(results[0].entry.id).toBe('vec-1'); // Most similar to itself
      expect(results[0].score).toBeCloseTo(1, 5);
    });

    it('should throw error for dimension mismatch on set', async () => {
      const wrongDimensionEmbedding = new Float32Array(256);

      await expect(store.set({
        id: 'vec-wrong',
        tenantId: 'tenant-001',
        sessionId: null,
        type: 'vector',
        key: 'wrong',
        value: {},
        embedding: wrongDimensionEmbedding
      })).rejects.toThrow('dimension mismatch');
    });

    it('should throw error for dimension mismatch on search', async () => {
      const wrongDimensionQuery = new Float32Array(256);

      await expect(store.search(wrongDimensionQuery, 'tenant-001'))
        .rejects.toThrow('dimension mismatch');
    });

    it('should only search within tenant', async () => {
      const embedding = createRandomEmbedding(384);

      await store.set({
        id: 'vec-1',
        tenantId: 'tenant-001',
        sessionId: null,
        type: 'vector',
        key: 'doc-1',
        value: {},
        embedding
      });

      await store.set({
        id: 'vec-2',
        tenantId: 'tenant-002',
        sessionId: null,
        type: 'vector',
        key: 'doc-2',
        value: {},
        embedding
      });

      const results = await store.search(embedding, 'tenant-001');

      expect(results).toHaveLength(1);
      expect(results[0].entry.tenantId).toBe('tenant-001');
    });
  });

  describe('Expiration', () => {
    it('should expire entries', async () => {
      const pastDate = new Date(Date.now() - 1000);
      const futureDate = new Date(Date.now() + 100000);

      await store.set({
        id: 'expired',
        tenantId: 'tenant-001',
        sessionId: null,
        type: 'short-term',
        key: 'expired',
        value: 'test',
        embedding: null,
        metadata: { expiresAt: pastDate }
      });

      await store.set({
        id: 'not-expired',
        tenantId: 'tenant-001',
        sessionId: null,
        type: 'short-term',
        key: 'not-expired',
        value: 'test',
        embedding: null,
        metadata: { expiresAt: futureDate }
      });

      const expiredCount = await store.expire();

      expect(expiredCount).toBe(1);
      expect(await store.get('expired')).toBeNull();
      expect(await store.get('not-expired')).not.toBeNull();
    });
  });

  describe('Clear Operations', () => {
    beforeEach(async () => {
      await store.set({
        id: 'mem-1',
        tenantId: 'tenant-001',
        sessionId: null,
        type: 'short-term',
        key: 'key-1',
        value: 'test',
        embedding: null
      });

      await store.set({
        id: 'mem-2',
        tenantId: 'tenant-002',
        sessionId: null,
        type: 'short-term',
        key: 'key-2',
        value: 'test',
        embedding: null
      });
    });

    it('should clear all entries', async () => {
      const cleared = await store.clear();

      expect(cleared).toBe(2);
      expect(store.size()).toBe(0);
    });

    it('should clear entries by tenant', async () => {
      const cleared = await store.clear('tenant-001');

      expect(cleared).toBe(1);
      expect(store.sizeByTenant('tenant-001')).toBe(0);
      expect(store.sizeByTenant('tenant-002')).toBe(1);
    });
  });

  describe('Size Operations', () => {
    it('should return total size', async () => {
      await store.set({
        id: 'mem-1',
        tenantId: 'tenant-001',
        sessionId: null,
        type: 'short-term',
        key: 'k1',
        value: 'v1',
        embedding: null
      });

      await store.set({
        id: 'mem-2',
        tenantId: 'tenant-001',
        sessionId: null,
        type: 'short-term',
        key: 'k2',
        value: 'v2',
        embedding: null
      });

      expect(store.size()).toBe(2);
    });

    it('should return size by tenant', async () => {
      await store.set({
        id: 'mem-1',
        tenantId: 'tenant-001',
        sessionId: null,
        type: 'short-term',
        key: 'k1',
        value: 'v1',
        embedding: null
      });

      await store.set({
        id: 'mem-2',
        tenantId: 'tenant-002',
        sessionId: null,
        type: 'short-term',
        key: 'k2',
        value: 'v2',
        embedding: null
      });

      expect(store.sizeByTenant('tenant-001')).toBe(1);
      expect(store.sizeByTenant('tenant-002')).toBe(1);
    });
  });
});

describe('Memory Factory Integration', () => {
  let store: MemoryStore;

  beforeEach(() => {
    store = new MemoryStore(384);
  });

  it('should create memory from factory data', async () => {
    const factoryMemory = createMemory({
      key: 'factory-key',
      value: { factory: 'data' },
      type: 'long-term'
    });

    const entry = await store.set({
      id: factoryMemory.id,
      tenantId: factoryMemory.tenantId,
      sessionId: factoryMemory.sessionId,
      type: factoryMemory.type,
      key: factoryMemory.key,
      value: factoryMemory.value,
      embedding: factoryMemory.embedding
    });

    expect(entry.key).toBe('factory-key');
    expect(entry.type).toBe('long-term');
  });

  it('should create vector memory from factory data', async () => {
    const factoryMemory = createVectorMemory(384, {
      key: 'vector-key',
      value: { text: 'Test document' }
    });

    const entry = await store.set({
      id: factoryMemory.id,
      tenantId: factoryMemory.tenantId,
      sessionId: factoryMemory.sessionId,
      type: factoryMemory.type,
      key: factoryMemory.key,
      value: factoryMemory.value,
      embedding: factoryMemory.embedding
    });

    expect(entry.embedding).not.toBeNull();
    expect(entry.embedding?.length).toBe(384);
    expect(entry.type).toBe('vector');
  });
});
