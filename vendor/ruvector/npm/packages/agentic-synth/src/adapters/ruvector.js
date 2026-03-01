/**
 * RuVector integration adapter
 * Uses native @ruvector/core NAPI-RS bindings when available,
 * falls back to in-memory simulation for environments without native support.
 */

let ruvectorCore = null;

// Try to load native ruvector bindings
async function loadRuvector() {
  if (ruvectorCore !== null) return ruvectorCore;

  try {
    // Try @ruvector/core first (native NAPI-RS bindings)
    const core = await import('@ruvector/core');
    ruvectorCore = core;
    return core;
  } catch (e1) {
    try {
      // Fall back to ruvector package
      const ruvector = await import('ruvector');
      ruvectorCore = ruvector;
      return ruvector;
    } catch (e2) {
      // No ruvector available
      ruvectorCore = false;
      return false;
    }
  }
}

export class RuvectorAdapter {
  constructor(options = {}) {
    this.vectorDb = null;
    this.dimensions = options.dimensions || 128;
    this.initialized = false;
    this.useNative = false;
    this.nativeDb = null;
    this.collectionName = options.collection || 'agentic-synth';
    this.inMemory = options.inMemory !== false; // Default to in-memory for tests
    this.path = options.path || null;
  }

  /**
   * Initialize RuVector connection
   * Attempts to use native bindings, falls back to in-memory simulation
   */
  async initialize() {
    try {
      const ruvector = await loadRuvector();

      if (ruvector && ruvector.VectorDB) {
        // Use native RuVector NAPI-RS bindings
        // VectorDB constructor takes { dimensions: number, path?: string }
        const dbOptions = { dimensions: this.dimensions };
        if (!this.inMemory && this.path) {
          dbOptions.path = this.path;
        }
        this.nativeDb = new ruvector.VectorDB(dbOptions);
        this.useNative = true;
        this.initialized = true;
        console.log('[RuvectorAdapter] Using native NAPI-RS bindings (in-memory:', this.inMemory, ')');
        return true;
      }

      // Fall back to in-memory simulation
      this.vectorDb = {
        vectors: new Map(),
        metadata: new Map(),
        config: { dimensions: this.dimensions }
      };
      this.useNative = false;
      this.initialized = true;
      console.log('[RuvectorAdapter] Using in-memory fallback (install @ruvector/core for native performance)');
      return true;
    } catch (error) {
      throw new Error(`Failed to initialize RuVector: ${error.message}`);
    }
  }

  /**
   * Insert vectors into database
   * @param {Array} vectors - Array of {id, vector, metadata?} objects
   */
  async insert(vectors) {
    if (!this.initialized) {
      throw new Error('RuVector adapter not initialized');
    }

    if (!Array.isArray(vectors)) {
      throw new Error('Vectors must be an array');
    }

    const results = [];

    if (this.useNative && this.nativeDb) {
      // Use native RuVector insert
      for (const item of vectors) {
        if (!item.id || !item.vector) {
          throw new Error('Each vector must have id and vector fields');
        }

        if (item.vector.length !== this.dimensions) {
          throw new Error(`Vector dimension mismatch: expected ${this.dimensions}, got ${item.vector.length}`);
        }

        // Native insert - takes { id, vector, metadata? }
        const vectorArray = item.vector instanceof Float32Array
          ? item.vector
          : new Float32Array(item.vector);

        this.nativeDb.insert({
          id: item.id,
          vector: vectorArray,
          metadata: item.metadata
        });
        results.push({ id: item.id, status: 'inserted', native: true });
      }
    } else {
      // In-memory fallback
      for (const item of vectors) {
        if (!item.id || !item.vector) {
          throw new Error('Each vector must have id and vector fields');
        }

        if (item.vector.length !== this.dimensions) {
          throw new Error(`Vector dimension mismatch: expected ${this.dimensions}, got ${item.vector.length}`);
        }

        this.vectorDb.vectors.set(item.id, item.vector);
        if (item.metadata) {
          this.vectorDb.metadata.set(item.id, item.metadata);
        }
        results.push({ id: item.id, status: 'inserted', native: false });
      }
    }

    return results;
  }

  /**
   * Batch insert for better performance
   * @param {Array} vectors - Array of {id, vector, metadata?} objects
   */
  async insertBatch(vectors) {
    if (!this.initialized) {
      throw new Error('RuVector adapter not initialized');
    }

    if (this.useNative && this.nativeDb && this.nativeDb.insertBatch) {
      // Use native batch insert if available
      const ids = vectors.map(v => v.id);
      const embeddings = vectors.map(v =>
        v.vector instanceof Float32Array ? v.vector : new Float32Array(v.vector)
      );
      const metadataList = vectors.map(v => v.metadata || {});

      this.nativeDb.insertBatch(ids, embeddings, metadataList);
      return vectors.map(v => ({ id: v.id, status: 'inserted', native: true }));
    }

    // Fall back to sequential insert
    return this.insert(vectors);
  }

  /**
   * Search for similar vectors
   * @param {Array|Float32Array} query - Query vector
   * @param {number} k - Number of results
   */
  async search(query, k = 10) {
    if (!this.initialized) {
      throw new Error('RuVector adapter not initialized');
    }

    const queryArray = Array.isArray(query) ? query : Array.from(query);

    if (queryArray.length !== this.dimensions) {
      throw new Error(`Query dimension mismatch: expected ${this.dimensions}, got ${queryArray.length}`);
    }

    if (this.useNative && this.nativeDb) {
      // Use native HNSW search - API: { vector, k }
      const queryFloat32 = query instanceof Float32Array ? query : new Float32Array(query);
      const results = await this.nativeDb.search({ vector: queryFloat32, k });
      return results.map(r => ({
        id: r.id,
        score: r.score || r.similarity || r.distance,
        metadata: r.metadata
      }));
    }

    // In-memory cosine similarity search
    const results = [];
    for (const [id, vector] of this.vectorDb.vectors.entries()) {
      const similarity = this._cosineSimilarity(queryArray, vector);
      results.push({
        id,
        score: similarity,
        metadata: this.vectorDb.metadata.get(id)
      });
    }

    results.sort((a, b) => b.score - a.score);
    return results.slice(0, k);
  }

  /**
   * Get vector by ID
   */
  async get(id) {
    if (!this.initialized) {
      throw new Error('RuVector adapter not initialized');
    }

    if (this.useNative && this.nativeDb && this.nativeDb.get) {
      const result = await this.nativeDb.get(id);
      return result ? { id: result.id, vector: result.vector, metadata: result.metadata } : null;
    }

    const vector = this.vectorDb.vectors.get(id);
    const metadata = this.vectorDb.metadata.get(id);
    return vector ? { id, vector, metadata } : null;
  }

  /**
   * Delete vector by ID
   */
  async delete(id) {
    if (!this.initialized) {
      throw new Error('RuVector adapter not initialized');
    }

    if (this.useNative && this.nativeDb && this.nativeDb.delete) {
      return await this.nativeDb.delete(id);
    }

    const existed = this.vectorDb.vectors.has(id);
    this.vectorDb.vectors.delete(id);
    this.vectorDb.metadata.delete(id);
    return existed;
  }

  /**
   * Get database statistics
   */
  async stats() {
    if (!this.initialized) {
      throw new Error('RuVector adapter not initialized');
    }

    if (this.useNative && this.nativeDb) {
      const count = await this.nativeDb.len();
      return {
        count,
        dimensions: this.dimensions,
        native: true
      };
    }

    return {
      count: this.vectorDb.vectors.size,
      dimensions: this.dimensions,
      native: false
    };
  }

  /**
   * Check if using native bindings
   */
  isNative() {
    return this.useNative;
  }

  /**
   * Calculate cosine similarity (fallback)
   * @private
   */
  _cosineSimilarity(a, b) {
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

/**
 * Create a RuVector adapter with automatic native detection
 */
export async function createRuvectorAdapter(options = {}) {
  const adapter = new RuvectorAdapter(options);
  await adapter.initialize();
  return adapter;
}

export default RuvectorAdapter;
